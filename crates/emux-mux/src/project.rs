//! Project-based workspace detection.
//!
//! Detects git repository roots and extracts project metadata so that
//! sessions can be organised by project context.

use std::path::{Path, PathBuf};

/// Detected project information derived from a git repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    /// Project root directory (the directory containing `.git`).
    pub root: PathBuf,
    /// Project name (the directory name of `root`).
    pub name: String,
    /// Current git branch, if available.
    pub branch: Option<String>,
}

/// Walk up from `start` looking for a `.git` directory or file.
///
/// Returns the directory that contains `.git`, i.e. the repository root.
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let git_path = current.join(".git");
        if git_path.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Parse `.git/HEAD` to extract the current branch name.
///
/// Handles two formats:
/// - Symbolic ref: `ref: refs/heads/<branch>`
/// - Detached HEAD: a raw commit hash (returns `None`).
pub fn read_git_branch(git_dir: &Path) -> Option<String> {
    let head_path = git_dir.join("HEAD");
    let contents = std::fs::read_to_string(head_path).ok()?;
    let trimmed = contents.trim();

    if let Some(ref_target) = trimmed.strip_prefix("ref: ") {
        // e.g. "refs/heads/main" -> "main"
        ref_target
            .strip_prefix("refs/heads/")
            .map(|b| b.to_string())
    } else {
        // Detached HEAD (raw SHA) — no branch name.
        None
    }
}

/// Detect project information from a working directory.
///
/// Walks up from `cwd` looking for a `.git` directory. If found, extracts
/// the project name from the root directory name and reads the current
/// branch from `.git/HEAD`.
pub fn detect_project(cwd: &Path) -> Option<ProjectInfo> {
    let root = find_git_root(cwd)?;
    let name = root
        .file_name()?
        .to_str()?
        .to_string();

    let git_dir = root.join(".git");
    // .git can be a file (worktrees / submodules) pointing elsewhere;
    // only read branch when it is a directory.
    let branch = if git_dir.is_dir() {
        read_git_branch(&git_dir)
    } else {
        None
    };

    Some(ProjectInfo {
        root,
        name,
        branch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn find_git_root_returns_none_for_tmp() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_git_root(dir.path()).is_none());
    }

    #[test]
    fn read_git_branch_symbolic_ref() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("HEAD"), "ref: refs/heads/feature/cool\n").unwrap();
        assert_eq!(read_git_branch(dir.path()), Some("feature/cool".to_string()));
    }

    #[test]
    fn read_git_branch_detached_head() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("HEAD"),
            "abc1234567890abcdef1234567890abcdef123456\n",
        )
        .unwrap();
        assert_eq!(read_git_branch(dir.path()), None);
    }

    #[test]
    fn detect_project_returns_none_without_git() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_project(dir.path()).is_none());
    }
}
