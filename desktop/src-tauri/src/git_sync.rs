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

/// Initialize a repo: ensure directory exists, create the 3 .md files if missing,
/// init git, commit, and push. If the repo already exists, just ensure the files exist.
pub fn init_repo(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let path = Path::new(local_path);
    let _ = std::fs::create_dir_all(path);

    // Create the 3 md files if they don't exist
    for name in &["todo.md", "today.md", "done.md"] {
        let file_path = path.join(name);
        if !file_path.exists() {
            std::fs::write(&file_path, "").map_err(|e| format!("Failed to create {}: {}", name, e))?;
        }
    }

    // Copy current settings into the repo
    let settings = crate::settings::load();
    let json = serde_json::to_string_pretty(&settings).unwrap_or_default();
    let _ = std::fs::write(path.join("settings.json"), json);

    if path.join(".git").exists() {
        // Repo already initialized — just ensure files are committed and pushed
        let repo = Repository::open(path).map_err(|e| format!("Failed to open repo: {}", e))?;
        // Check if there are uncommitted files
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).map_err(|e| format!("Status error: {}", e))?;
        if !statuses.is_empty() {
            // Stage and commit
            let mut index = repo.index().map_err(|e| format!("Index error: {}", e))?;
            index.add_all(["*.md", "settings.json"].iter(), git2::IndexAddOption::DEFAULT, None)
                .map_err(|e| format!("Add error: {}", e))?;
            index.write().map_err(|e| format!("Index write error: {}", e))?;
            let tree_oid = index.write_tree().map_err(|e| format!("Write tree error: {}", e))?;
            let tree = repo.find_tree(tree_oid).map_err(|e| format!("Find tree error: {}", e))?;
            let sig = Signature::now("Tally.md", "tally@local").map_err(|e| format!("Sig error: {}", e))?;
            let parents = match repo.head().and_then(|h| h.peel_to_commit()) {
                Ok(p) => vec![p],
                Err(_) => vec![],
            };
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
            repo.commit(Some("HEAD"), &sig, &sig, "Add missing files", &tree, &parent_refs)
                .map_err(|e| format!("Commit error: {}", e))?;
            do_push(&repo, token)?;
        }
        return Ok("Repo already initialized".to_string());
    }

    // Fresh init
    init_and_push(path, repo_url, token)?;
    Ok("Repo initialized and pushed".to_string())
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

        match builder.clone(repo_url, path) {
            Ok(repo) => Ok(repo),
            Err(e) => {
                // Clone fails on empty repos — init locally and set remote instead
                if e.message().contains("not found") || e.message().contains("empty") {
                    let _ = std::fs::create_dir_all(path);
                    init_and_push(path, repo_url, token)
                } else {
                    Err(format!("Failed to clone: {}", e))
                }
            }
        }
    }
}

/// Initialize a repo from existing files and push.
fn init_and_push(path: &Path, repo_url: &str, token: &str) -> Result<Repository, String> {
    let repo = Repository::init(path).map_err(|e| format!("Failed to init repo: {}", e))?;

    // Set HEAD to main (git2 defaults to master)
    repo.set_head("refs/heads/main")
        .map_err(|e| format!("Set head error: {}", e))?;

    repo.remote("origin", repo_url)
        .map_err(|e| format!("Failed to add remote: {}", e))?;

    // Stage all tracked files
    let mut index = repo.index().map_err(|e| format!("Index error: {}", e))?;
    index
        .add_all(["*.md", "settings.json"].iter(), git2::IndexAddOption::DEFAULT, None)
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

    // Commit to refs/heads/main explicitly
    repo.commit(Some("refs/heads/main"), &sig, &sig, "Initial commit from Tally.md", &tree, &[])
        .map_err(|e| format!("Commit error: {}", e))?;

    drop(tree);

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
    let fetch_head = match repo
        .find_reference(&format!("refs/remotes/origin/{}", default_branch))
    {
        Ok(r) => r,
        Err(_) => {
            // Remote branch doesn't exist yet (empty repo) — nothing to pull
            return Ok("Remote is empty, nothing to pull".to_string());
        }
    };
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

    // Backup local files before merge in case of conflicts
    let path = Path::new(local_path);
    let backup_dir = path.join(".backup");
    let _ = std::fs::create_dir_all(&backup_dir);
    let mut backed_up = Vec::new();
    for name in &["todo.md", "today.md", "done.md"] {
        let src = path.join(name);
        let dst = backup_dir.join(name);
        if src.exists() {
            if let Ok(_) = std::fs::copy(&src, &dst) {
                backed_up.push(name.to_string());
            }
        }
    }

    // Normal merge — force-checkout theirs on conflict
    let their_commit = repo
        .find_commit(fetch_commit.id())
        .map_err(|e| format!("Find commit error: {}", e))?;
    repo.merge(
        &[&fetch_commit],
        None,
        Some(git2::build::CheckoutBuilder::default().force()),
    )
    .map_err(|e| format!("Merge error: {}", e))?;

    // Check if index has conflicts
    let index_conflicts = repo.index()
        .ok()
        .map(|idx| idx.has_conflicts())
        .unwrap_or(false);

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

    // Detect if local files changed after merge (content differs from backup)
    let mut had_conflicts = index_conflicts;
    if !had_conflicts {
        for name in &["todo.md", "today.md", "done.md"] {
            let current = std::fs::read_to_string(path.join(name)).unwrap_or_default();
            let backup = std::fs::read_to_string(backup_dir.join(name)).unwrap_or_default();
            if current != backup {
                had_conflicts = true;
                break;
            }
        }
    }

    if had_conflicts {
        let backup_path = backup_dir.to_string_lossy().to_string();
        Ok(format!("Pulled (merged with conflicts). Local backup at {}", backup_path))
    } else {
        // Clean up backup if no conflicts
        let _ = std::fs::remove_dir_all(&backup_dir);
        Ok("Pulled (merged)".to_string())
    }
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

    // Stage all tracked files
    let mut index = repo.index().map_err(|e| format!("Index error: {}", e))?;
    index
        .add_all(["*.md", "settings.json"].iter(), git2::IndexAddOption::DEFAULT, None)
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

    // Use local HEAD branch — more reliable than remote detection for new repos
    let default_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "main".to_string());

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
    if let Err(_) = remote.connect_auth(Direction::Fetch, Some(callbacks), None) {
        // Can't connect — fall back to "main"
        return Ok("main".to_string());
    }

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
