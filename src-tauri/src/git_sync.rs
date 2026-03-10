use git2::{
    Cred, Direction, FetchOptions, PushOptions, RemoteCallbacks, Repository, Signature,
    StatusOptions,
};
use std::path::Path;

const KEYRING_SERVICE: &str = "com.sanderkohnstamm.tallymd";
const KEYRING_USER: &str = "git-token";

/// Store a PAT in the OS keychain.
pub fn store_token(token: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring init error: {}", e))?;
    entry
        .set_password(token)
        .map_err(|e| format!("Failed to store token: {}", e))
}

/// Retrieve the PAT from the OS keychain.
pub fn get_token() -> Result<String, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring init error: {}", e))?;
    entry
        .get_password()
        .map_err(|e| format!("No token stored: {}", e))
}

/// Check if a token is stored.
pub fn has_token() -> bool {
    get_token().is_ok()
}

/// Delete token from keychain.
pub fn delete_token() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring init error: {}", e))?;
    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete token: {}", e))
}

fn make_callbacks<'a>(token: &'a str) -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        let user = username_from_url.unwrap_or("git");
        Cred::userpass_plaintext(user, token)
    });
    callbacks
}

/// Clone a repo if local_path doesn't exist or is empty, otherwise open it.
pub fn ensure_repo(repo_url: &str, local_path: &str, token: &str) -> Result<Repository, String> {
    let path = Path::new(local_path);

    if path.join(".git").exists() {
        Repository::open(path).map_err(|e| format!("Failed to open repo: {}", e))
    } else {
        // Clone
        let callbacks = make_callbacks(token);
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        // Ensure parent exists
        let _ = std::fs::create_dir_all(path);
        // If directory exists but is empty or has no .git, remove and clone
        if path.exists() {
            let is_empty = path
                .read_dir()
                .map(|mut d| d.next().is_none())
                .unwrap_or(true);
            if !is_empty {
                // Directory has files but no .git — init and set remote
                return init_and_push(path, repo_url, token);
            }
            let _ = std::fs::remove_dir(path);
        }

        builder
            .clone(repo_url, path)
            .map_err(|e| format!("Failed to clone: {}", e))
    }
}

/// Initialize a repo from existing files and push.
fn init_and_push(path: &Path, repo_url: &str, token: &str) -> Result<Repository, String> {
    let repo = Repository::init(path).map_err(|e| format!("Failed to init repo: {}", e))?;

    repo.remote("origin", repo_url)
        .map_err(|e| format!("Failed to add remote: {}", e))?;

    // Stage all markdown files
    let mut index = repo.index().map_err(|e| format!("Index error: {}", e))?;
    index
        .add_all(["*.md"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| format!("Failed to add files: {}", e))?;
    index.write().map_err(|e| format!("Index write error: {}", e))?;

    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {}", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {}", e))?;

    let sig = Signature::now("Tally.md", "tally@local")
        .map_err(|e| format!("Signature error: {}", e))?;

    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit from Tally.md", &tree, &[])
        .map_err(|e| format!("Commit error: {}", e))?;

    // Push
    do_push(&repo, token)?;

    Ok(repo)
}

/// Pull (fetch + merge) from origin/main.
pub fn pull(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let repo = ensure_repo(repo_url, local_path, token)?;

    let callbacks = make_callbacks(token);
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No remote: {}", e))?;

    // Detect the default branch
    let default_branch = detect_default_branch(&mut remote, token)?;

    let callbacks2 = make_callbacks(token);
    let mut fo2 = FetchOptions::new();
    fo2.remote_callbacks(callbacks2);
    remote
        .fetch(&[&default_branch], Some(&mut fo2), None)
        .map_err(|e| format!("Fetch failed: {}", e))?;

    // Merge
    let fetch_head = repo
        .find_reference(&format!("refs/remotes/origin/{}", default_branch))
        .map_err(|e| format!("No remote ref: {}", e))?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| format!("Annotated commit error: {}", e))?;

    let (analysis, _) = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|e| format!("Merge analysis error: {}", e))?;

    if analysis.is_up_to_date() {
        return Ok("Already up to date".to_string());
    }

    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{}", default_branch);
        if let Ok(mut reference) = repo.find_reference(&refname) {
            reference
                .set_target(fetch_commit.id(), "Fast-forward")
                .map_err(|e| format!("FF error: {}", e))?;
        } else {
            repo.reference(&refname, fetch_commit.id(), true, "Fast-forward")
                .map_err(|e| format!("Ref create error: {}", e))?;
        }
        repo.set_head(&refname)
            .map_err(|e| format!("Set head error: {}", e))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Checkout error: {}", e))?;
        return Ok("Pulled (fast-forward)".to_string());
    }

    // Normal merge — for simplicity, force-checkout theirs on conflict
    let their_commit = repo
        .find_commit(fetch_commit.id())
        .map_err(|e| format!("Find commit error: {}", e))?;
    repo.merge(
        &[&fetch_commit],
        None,
        Some(git2::build::CheckoutBuilder::default().force()),
    )
    .map_err(|e| format!("Merge error: {}", e))?;

    // Auto-commit the merge
    let sig = Signature::now("Tally.md", "tally@local")
        .map_err(|e| format!("Signature error: {}", e))?;
    let mut index = repo.index().map_err(|e| format!("Index error: {}", e))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {}", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {}", e))?;
    let head_commit = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|e| format!("Head error: {}", e))?;

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Merge from remote",
        &tree,
        &[&head_commit, &their_commit],
    )
    .map_err(|e| format!("Merge commit error: {}", e))?;

    repo.cleanup_state()
        .map_err(|e| format!("Cleanup error: {}", e))?;

    Ok("Pulled (merged)".to_string())
}

/// Commit all changed .md files and push.
pub fn commit_and_push(
    repo_url: &str,
    local_path: &str,
    token: &str,
) -> Result<String, String> {
    let repo = ensure_repo(repo_url, local_path, token)?;

    // Check for changes
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Status error: {}", e))?;

    if statuses.is_empty() {
        return Ok("Nothing to sync".to_string());
    }

    // Stage all .md files
    let mut index = repo.index().map_err(|e| format!("Index error: {}", e))?;
    index
        .add_all(["*.md"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| format!("Add error: {}", e))?;
    index.write().map_err(|e| format!("Index write error: {}", e))?;

    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {}", e))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {}", e))?;

    let sig = Signature::now("Tally.md", "tally@local")
        .map_err(|e| format!("Signature error: {}", e))?;

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
    let message = format!("Tally.md sync {}", timestamp);

    let parents = match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(parent) => vec![parent],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &parent_refs)
        .map_err(|e| format!("Commit error: {}", e))?;

    do_push(&repo, token)?;

    Ok(format!("Synced: {}", message))
}

fn do_push(repo: &Repository, token: &str) -> Result<(), String> {
    let callbacks = make_callbacks(token);
    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No remote: {}", e))?;

    // Detect default branch for push
    let callbacks2 = make_callbacks(token);
    let default_branch = detect_default_branch(&mut remote, token)?;

    let callbacks3 = make_callbacks(token);
    let mut push_opts2 = PushOptions::new();
    push_opts2.remote_callbacks(callbacks3);

    remote
        .push(
            &[&format!("refs/heads/{}:refs/heads/{}", default_branch, default_branch)],
            Some(&mut push_opts2),
        )
        .map_err(|e| format!("Push failed: {}", e))
}

fn detect_default_branch(remote: &mut git2::Remote, token: &str) -> Result<String, String> {
    let callbacks = make_callbacks(token);
    remote
        .connect_auth(Direction::Fetch, Some(callbacks), None)
        .map_err(|e| format!("Connect error: {}", e))?;

    let default = remote
        .default_branch()
        .ok()
        .and_then(|b| b.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "refs/heads/main".to_string());

    remote.disconnect().ok();

    // Strip refs/heads/ prefix
    let branch = default
        .strip_prefix("refs/heads/")
        .unwrap_or(&default)
        .to_string();

    Ok(branch)
}
