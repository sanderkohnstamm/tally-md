use git2::{
    Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository, Signature, StatusOptions,
};
use std::path::Path;

// --- Token storage: keyring for release, file-based for debug ---

#[cfg(not(debug_assertions))]
const KEYRING_SERVICE: &str = "com.sanderkohnstamm.tallymd";
#[cfg(not(debug_assertions))]
const KEYRING_USER: &str = "git-token";

#[cfg(not(debug_assertions))]
pub fn store_token(token: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring init error: {e}"))?;
    entry
        .set_password(token)
        .map_err(|e| format!("Failed to store token: {e}"))
}

#[cfg(not(debug_assertions))]
pub fn get_token() -> Result<String, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring init error: {e}"))?;
    entry
        .get_password()
        .map_err(|e| format!("No token stored: {e}"))
}

#[cfg(not(debug_assertions))]
pub fn has_token() -> bool {
    get_token().is_ok()
}

#[cfg(not(debug_assertions))]
pub fn delete_token() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring init error: {e}"))?;
    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete token: {e}"))
}

// Debug builds: file-based token at ~/.tallymd/.token (avoids keychain prompts)

#[cfg(debug_assertions)]
fn token_path() -> std::path::PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".tallymd");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(".token")
}

#[cfg(debug_assertions)]
pub fn store_token(token: &str) -> Result<(), String> {
    std::fs::write(token_path(), token).map_err(|e| format!("Failed to store token: {e}"))
}

#[cfg(debug_assertions)]
pub fn get_token() -> Result<String, String> {
    let token = std::fs::read_to_string(token_path()).map_err(|_| "No token stored".to_string())?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("No token stored".to_string());
    }
    Ok(token)
}

#[cfg(debug_assertions)]
pub fn has_token() -> bool {
    get_token().is_ok()
}

#[cfg(debug_assertions)]
pub fn delete_token() -> Result<(), String> {
    let path = token_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete token: {e}"))?;
    }
    Ok(())
}

fn make_callbacks(token: &str) -> RemoteCallbacks<'_> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        let user = username_from_url.unwrap_or("git");
        Cred::userpass_plaintext(user, token)
    });
    callbacks
}

/// Ensure the 4 expected files exist, creating only the missing ones.
fn ensure_files_exist(path: &Path) -> Result<(), String> {
    for name in &["todo.md", "today.md", "done.md"] {
        let file_path = path.join(name);
        if !file_path.exists() {
            std::fs::write(&file_path, "").map_err(|e| format!("Failed to create {name}: {e}"))?;
        }
    }
    if !path.join("settings.json").exists() {
        let settings = crate::settings::load();
        let json = serde_json::to_string_pretty(&settings).unwrap_or_default();
        std::fs::write(path.join("settings.json"), &json)
            .map_err(|e| format!("Failed to write settings.json: {e}"))?;
    }
    Ok(())
}

/// Stage and commit any dirty files. Returns whether a commit was made.
fn commit_if_dirty(repo: &Repository, token: &str, message: &str) -> Result<bool, String> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Status error: {e}"))?;

    if statuses.is_empty() {
        return Ok(false);
    }

    let mut index = repo.index().map_err(|e| format!("Index error: {e}"))?;
    index
        .add_all(
            ["*.md", "settings.json"].iter(),
            git2::IndexAddOption::DEFAULT,
            None,
        )
        .map_err(|e| format!("Add error: {e}"))?;
    index
        .write()
        .map_err(|e| format!("Index write error: {e}"))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {e}"))?;
    let sig = Signature::now("Tally.md", "tally@local").map_err(|e| format!("Sig error: {e}"))?;
    let parents = match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(p) => vec![p],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .map_err(|e| format!("Commit error: {e}"))?;
    do_push(repo, token)?;
    Ok(true)
}

/// Ensure the repo is on branch "main". If HEAD points to another branch, rename it.
fn ensure_main_branch(repo: &Repository) -> Result<(), String> {
    let Ok(head) = repo.head() else {
        return Ok(());
    };
    let branch_name = head.shorthand().unwrap_or("main").to_string();
    if branch_name == "main" {
        return Ok(());
    }
    let mut branch = repo
        .find_branch(&branch_name, git2::BranchType::Local)
        .map_err(|e| format!("Find branch error: {e}"))?;
    branch
        .rename("main", true)
        .map_err(|e| format!("Rename branch to main: {e}"))?;
    Ok(())
}

/// Initialize a repo: if the remote already exists, clone it and only add missing files.
/// If no remote content, init fresh. Always uses branch "main".
pub fn init_repo(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let path = Path::new(local_path);
    let _ = std::fs::create_dir_all(path);

    // Already have a local git repo — just ensure files exist and sync
    if path.join(".git").exists() {
        let repo = Repository::open(path).map_err(|e| format!("Failed to open repo: {e}"))?;
        ensure_main_branch(&repo)?;
        ensure_files_exist(path)?;
        commit_if_dirty(&repo, token, "Add missing files")?;
        return Ok("Repo already initialized".to_string());
    }

    // Try to clone the existing remote repo first
    let clone_result = {
        let callbacks = make_callbacks(token);
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(callbacks);
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);
        builder.branch("main");
        // git2 needs an empty or non-existent directory to clone into
        let _ = std::fs::remove_dir(path);
        builder.clone(repo_url, path)
    };

    if let Ok(repo) = clone_result {
        ensure_main_branch(&repo)?;
        ensure_files_exist(path)?;
        commit_if_dirty(&repo, token, "Add missing files")?;
        Ok("Cloned existing repo".to_string())
    } else {
        // Clone failed (empty repo or doesn't exist yet) — init fresh
        let _ = std::fs::create_dir_all(path);
        ensure_files_exist(path)?;
        init_and_push(path, repo_url, token)?;
        Ok("Repo initialized and pushed".to_string())
    }
}

/// Clone a repo if `local_path` doesn't exist or is empty, otherwise open it.
pub fn ensure_repo(repo_url: &str, local_path: &str, token: &str) -> Result<Repository, String> {
    let path = Path::new(local_path);

    if path.join(".git").exists() {
        Repository::open(path).map_err(|e| format!("Failed to open repo: {e}"))
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
                    Err(format!("Failed to clone: {e}"))
                }
            }
        }
    }
}

/// Initialize a repo from existing files and push.
fn init_and_push(path: &Path, repo_url: &str, token: &str) -> Result<Repository, String> {
    let repo = Repository::init(path).map_err(|e| format!("Failed to init repo: {e}"))?;

    // Set HEAD to main (git2 defaults to master)
    repo.set_head("refs/heads/main")
        .map_err(|e| format!("Set head error: {e}"))?;

    repo.remote("origin", repo_url)
        .map_err(|e| format!("Failed to add remote: {e}"))?;

    // Stage all tracked files
    let mut index = repo.index().map_err(|e| format!("Index error: {e}"))?;
    index
        .add_all(
            ["*.md", "settings.json"].iter(),
            git2::IndexAddOption::DEFAULT,
            None,
        )
        .map_err(|e| format!("Failed to add files: {e}"))?;
    index
        .write()
        .map_err(|e| format!("Index write error: {e}"))?;

    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {e}"))?;

    let sig =
        Signature::now("Tally.md", "tally@local").map_err(|e| format!("Signature error: {e}"))?;

    // Commit to refs/heads/main explicitly
    repo.commit(
        Some("refs/heads/main"),
        &sig,
        &sig,
        "Initial commit from Tally.md",
        &tree,
        &[],
    )
    .map_err(|e| format!("Commit error: {e}"))?;

    drop(tree);

    // Push
    do_push(&repo, token)?;

    Ok(repo)
}

/// Pull (fetch + merge) from origin/main.
#[allow(clippy::too_many_lines)]
pub fn pull(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let repo = ensure_repo(repo_url, local_path, token)?;

    let callbacks = make_callbacks(token);
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No remote: {e}"))?;

    // Detect the default branch
    let default_branch = detect_default_branch(&mut remote, token)?;

    let callbacks2 = make_callbacks(token);
    let mut fo2 = FetchOptions::new();
    fo2.remote_callbacks(callbacks2);
    remote
        .fetch(&[&default_branch], Some(&mut fo2), None)
        .map_err(|e| format!("Fetch failed: {e}"))?;

    // Merge
    let Ok(fetch_head) = repo.find_reference(&format!("refs/remotes/origin/{default_branch}"))
    else {
        // Remote branch doesn't exist yet (empty repo) — nothing to pull
        return Ok("Remote is empty, nothing to pull".to_string());
    };
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| format!("Annotated commit error: {e}"))?;

    let (analysis, _) = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|e| format!("Merge analysis error: {e}"))?;

    if analysis.is_up_to_date() {
        return Ok("Already up to date".to_string());
    }

    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{default_branch}");
        if let Ok(mut reference) = repo.find_reference(&refname) {
            reference
                .set_target(fetch_commit.id(), "Fast-forward")
                .map_err(|e| format!("FF error: {e}"))?;
        } else {
            repo.reference(&refname, fetch_commit.id(), true, "Fast-forward")
                .map_err(|e| format!("Ref create error: {e}"))?;
        }
        repo.set_head(&refname)
            .map_err(|e| format!("Set head error: {e}"))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| format!("Checkout error: {e}"))?;
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
        if src.exists() && std::fs::copy(&src, &dst).is_ok() {
            backed_up.push((*name).to_string());
        }
    }

    // Normal merge — auto-resolve conflicts favoring local (ours) to prevent conflict markers
    let their_commit = repo
        .find_commit(fetch_commit.id())
        .map_err(|e| format!("Find commit error: {e}"))?;
    let mut merge_opts = git2::MergeOptions::new();
    merge_opts.file_favor(git2::FileFavor::Ours);
    repo.merge(
        &[&fetch_commit],
        Some(&mut merge_opts),
        Some(git2::build::CheckoutBuilder::default().force()),
    )
    .map_err(|e| format!("Merge error: {e}"))?;

    // Resolve any remaining index conflicts (shouldn't happen with FileFavor::Ours, but be safe)
    {
        let mut index = repo.index().map_err(|e| format!("Index error: {e}"))?;
        if index.has_conflicts() {
            let conflicts: Vec<_> = index
                .conflicts()
                .map_err(|e| format!("Conflicts iter error: {e}"))?
                .filter_map(std::result::Result::ok)
                .collect();
            for conflict in &conflicts {
                // Prefer "ours" (local) version; fall back to "theirs" if ours doesn't exist
                if let Some(ref ours) = conflict.our {
                    index
                        .add(ours)
                        .map_err(|e| format!("Resolve conflict error: {e}"))?;
                } else if let Some(ref theirs) = conflict.their {
                    index
                        .add(theirs)
                        .map_err(|e| format!("Resolve conflict error: {e}"))?;
                }
            }
            index
                .write()
                .map_err(|e| format!("Index write error: {e}"))?;
        }
    }

    // Check if local files changed after merge (content differs from backup)
    let index_conflicts = false; // Conflicts are now auto-resolved

    // Auto-commit the merge
    let sig =
        Signature::now("Tally.md", "tally@local").map_err(|e| format!("Signature error: {e}"))?;
    let mut index = repo.index().map_err(|e| format!("Index error: {e}"))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {e}"))?;
    let head_commit = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|e| format!("Head error: {e}"))?;

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Merge from remote",
        &tree,
        &[&head_commit, &their_commit],
    )
    .map_err(|e| format!("Merge commit error: {e}"))?;

    repo.cleanup_state()
        .map_err(|e| format!("Cleanup error: {e}"))?;

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
        Ok(format!(
            "Pulled (merged with conflicts). Local backup at {backup_path}"
        ))
    } else {
        // Clean up backup if no conflicts
        let _ = std::fs::remove_dir_all(&backup_dir);
        Ok("Pulled (merged)".to_string())
    }
}

/// Commit all changed .md files and push.
pub fn commit_and_push(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let repo = ensure_repo(repo_url, local_path, token)?;

    // Check for changes
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Status error: {e}"))?;

    if statuses.is_empty() {
        return Ok("Nothing to sync".to_string());
    }

    // Stage all tracked files
    let mut index = repo.index().map_err(|e| format!("Index error: {e}"))?;
    index
        .add_all(
            ["*.md", "settings.json"].iter(),
            git2::IndexAddOption::DEFAULT,
            None,
        )
        .map_err(|e| format!("Add error: {e}"))?;
    index
        .write()
        .map_err(|e| format!("Index write error: {e}"))?;

    let tree_oid = index
        .write_tree()
        .map_err(|e| format!("Write tree error: {e}"))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| format!("Find tree error: {e}"))?;

    let sig =
        Signature::now("Tally.md", "tally@local").map_err(|e| format!("Signature error: {e}"))?;

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M");
    let message = format!("Tally.md sync {timestamp}");

    let parents = match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(parent) => vec![parent],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &parent_refs)
        .map_err(|e| format!("Commit error: {e}"))?;

    do_push(&repo, token)?;

    Ok(format!("Synced: {message}"))
}

fn do_push(repo: &Repository, token: &str) -> Result<(), String> {
    let callbacks = make_callbacks(token);
    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No remote: {e}"))?;

    // Use local HEAD branch — more reliable than remote detection for new repos
    let default_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(ToString::to_string))
        .unwrap_or_else(|| "main".to_string());

    let callbacks3 = make_callbacks(token);
    let mut push_opts2 = PushOptions::new();
    push_opts2.remote_callbacks(callbacks3);

    remote
        .push(
            &[&format!(
                "refs/heads/{default_branch}:refs/heads/{default_branch}"
            )],
            Some(&mut push_opts2),
        )
        .map_err(|e| format!("Push failed: {e}"))
}

/// Always use "main" as the branch name.
#[allow(clippy::unnecessary_wraps)]
fn detect_default_branch(_remote: &mut git2::Remote, _token: &str) -> Result<String, String> {
    Ok("main".to_string())
}

/// Force pull: discard local changes and reset to remote version.
pub fn force_pull(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let repo = ensure_repo(repo_url, local_path, token)?;

    let callbacks = make_callbacks(token);
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks);

    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No remote: {e}"))?;

    let default_branch = detect_default_branch(&mut remote, token)?;

    let callbacks2 = make_callbacks(token);
    let mut fo2 = FetchOptions::new();
    fo2.remote_callbacks(callbacks2);
    remote
        .fetch(&[&default_branch], Some(&mut fo2), None)
        .map_err(|e| format!("Fetch failed: {e}"))?;

    let fetch_ref = repo
        .find_reference(&format!("refs/remotes/origin/{default_branch}"))
        .map_err(|e| format!("No remote branch: {e}"))?;
    let fetch_commit = fetch_ref
        .peel_to_commit()
        .map_err(|e| format!("Peel error: {e}"))?;

    // Reset HEAD to remote commit
    let refname = format!("refs/heads/{default_branch}");
    repo.reference(
        &refname,
        fetch_commit.id(),
        true,
        "Force pull: reset to remote",
    )
    .map_err(|e| format!("Ref update error: {e}"))?;
    repo.set_head(&refname)
        .map_err(|e| format!("Set head error: {e}"))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        .map_err(|e| format!("Checkout error: {e}"))?;

    // Clean merge state if any
    let _ = repo.cleanup_state();

    Ok("Force pulled — local files reset to remote version".to_string())
}

/// Force push: overwrite remote with current local version.
pub fn force_push(repo_url: &str, local_path: &str, token: &str) -> Result<String, String> {
    let repo = ensure_repo(repo_url, local_path, token)?;

    // Stage and commit any pending changes
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| format!("Status error: {e}"))?;

    if !statuses.is_empty() {
        let mut index = repo.index().map_err(|e| format!("Index error: {e}"))?;
        index
            .add_all(
                ["*.md", "settings.json"].iter(),
                git2::IndexAddOption::DEFAULT,
                None,
            )
            .map_err(|e| format!("Add error: {e}"))?;
        index
            .write()
            .map_err(|e| format!("Index write error: {e}"))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| format!("Write tree error: {e}"))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| format!("Find tree error: {e}"))?;
        let sig =
            Signature::now("Tally.md", "tally@local").map_err(|e| format!("Sig error: {e}"))?;
        let parents = match repo.head().and_then(|h| h.peel_to_commit()) {
            Ok(p) => vec![p],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Force push from Tally.md",
            &tree,
            &parent_refs,
        )
        .map_err(|e| format!("Commit error: {e}"))?;
    }

    // Force push
    let callbacks = make_callbacks(token);
    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| format!("No remote: {e}"))?;

    let default_branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(ToString::to_string))
        .unwrap_or_else(|| "main".to_string());

    remote
        .push(
            &[&format!(
                "+refs/heads/{default_branch}:refs/heads/{default_branch}"
            )],
            Some(&mut push_opts),
        )
        .map_err(|e| format!("Force push failed: {e}"))?;

    Ok("Force pushed — remote now matches local version".to_string())
}
