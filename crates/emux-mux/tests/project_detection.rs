use std::fs;
use std::path::PathBuf;

use emux_mux::project::{detect_project, find_git_root, read_git_branch};
use emux_mux::{Pane, ProjectInfo, Session};

/// We are running inside the emux repo, so this should find the repo root.
#[test]
fn test_find_git_root_in_repo() {
    let cwd = std::env::current_dir().unwrap();
    let root = find_git_root(&cwd).expect("should find repo root");
    assert!(root.join(".git").exists());
}

/// Starting at the repo root itself should still succeed.
#[test]
fn test_find_git_root_at_root() {
    // Walk up from cwd to find the root first, then verify find_git_root
    // returns the same path when called at the root.
    let cwd = std::env::current_dir().unwrap();
    let root = find_git_root(&cwd).unwrap();
    let again = find_git_root(&root).expect("should find root when starting at root");
    assert_eq!(root, again);
}

/// A directory with no `.git` ancestor should return None.
#[test]
fn test_find_git_root_nonexistent() {
    let dir = std::env::temp_dir().join("emux_project_test_no_git");
    let _ = fs::create_dir_all(&dir);
    assert!(find_git_root(&dir).is_none());
    let _ = fs::remove_dir_all(&dir);
}

/// Parse a symbolic ref from a fake .git/HEAD.
#[test]
fn test_read_git_branch() {
    let dir = std::env::temp_dir().join("emux_read_branch_test");
    let _ = fs::create_dir_all(&dir);
    fs::write(dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    assert_eq!(read_git_branch(&dir), Some("main".to_string()));
    let _ = fs::remove_dir_all(&dir);
}

/// A detached HEAD (raw SHA) should return None.
#[test]
fn test_read_git_branch_detached() {
    let dir = std::env::temp_dir().join("emux_detached_branch_test");
    let _ = fs::create_dir_all(&dir);
    fs::write(
        dir.join("HEAD"),
        "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n",
    )
    .unwrap();
    assert_eq!(read_git_branch(&dir), None);
    let _ = fs::remove_dir_all(&dir);
}

/// Full integration: detect_project inside the emux repo.
#[test]
fn test_detect_project_full() {
    let cwd = std::env::current_dir().unwrap();
    let info = detect_project(&cwd).expect("should detect project");
    assert!(!info.name.is_empty());
    assert!(info.root.join(".git").exists());
    // Branch may or may not be present (CI could use detached HEAD),
    // but the field should at least be parseable.
}

/// Verify the project name is extracted from the directory name.
#[test]
fn test_project_name_from_dir() {
    let base = std::env::temp_dir().join("emux_proj_name_test");
    let project_dir = base.join("my-cool-project");
    let git_dir = project_dir.join(".git");
    let _ = fs::create_dir_all(&git_dir);
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/develop\n").unwrap();

    let info = detect_project(&project_dir).expect("should detect project");
    assert_eq!(info.name, "my-cool-project");
    assert_eq!(info.branch, Some("develop".to_string()));
    assert_eq!(info.root, project_dir);

    let _ = fs::remove_dir_all(&base);
}

/// Test Session.set_project / project_name / git_branch.
#[test]
fn test_session_project_info() {
    let mut session = Session::new("dev", 80, 25);
    assert!(session.project_name().is_none());
    assert!(session.git_branch().is_none());

    let info = ProjectInfo {
        root: PathBuf::from("/home/user/projects/foo"),
        name: "foo".to_string(),
        branch: Some("feature/bar".to_string()),
    };
    session.set_project(info);

    assert_eq!(session.project_name(), Some("foo"));
    assert_eq!(session.git_branch(), Some("feature/bar"));
    assert_eq!(
        session.project().unwrap().root,
        PathBuf::from("/home/user/projects/foo")
    );
}

/// Test Pane working directory tracking.
#[test]
fn test_pane_working_directory() {
    let mut pane = Pane::new(0, 80, 25);
    assert!(pane.working_directory().is_none());

    pane.set_working_directory(PathBuf::from("/home/user/projects/emux"));
    assert_eq!(
        pane.working_directory(),
        Some(std::path::Path::new("/home/user/projects/emux"))
    );

    // Update to a new directory.
    pane.set_working_directory(PathBuf::from("/tmp"));
    assert_eq!(
        pane.working_directory(),
        Some(std::path::Path::new("/tmp"))
    );
}
