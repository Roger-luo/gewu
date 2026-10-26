use std::{fs, path::Path, process::Command};

fn conventional_commit_hook() -> String {
    let config = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../prek.toml"))
        .expect("read repository hook configuration");
    let hook = config
        .split("id = \"conventional-commit\"")
        .nth(1)
        .and_then(|section| {
            section
                .lines()
                .find_map(|line| line.strip_prefix("entry = '''"))
        })
        .and_then(|entry| entry.strip_suffix("'''"))
        .expect("conventional commit hook entry");
    hook.to_owned()
}

fn hook_accepts(hook: &str, message: &str) -> bool {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("COMMIT_EDITMSG");
    fs::write(&path, format!("{message}\n\nBody ignored by the hook.\n"))
        .expect("write commit message");
    Command::new("sh")
        .args(["-c", &format!("{hook} \"$1\""), "--"])
        .arg(path)
        .status()
        .expect("run conventional commit hook")
        .success()
}

#[test]
fn conventional_commit_hook_accepts_documented_forms_and_rejects_bad_scopes() {
    let hook = conventional_commit_hook();

    for message in [
        "feat: add review scope prompt",
        "fix(review): reject invalid roots",
        "docs(path/to)!: explain migration",
    ] {
        assert!(hook_accepts(&hook, message), "should accept {message:?}");
    }
    for message in [
        "fix(path\\to): reject this scope",
        "squash! feat: temporary commit",
        "change(review): unknown type",
        "fix(review):",
    ] {
        assert!(!hook_accepts(&hook, message), "should reject {message:?}");
    }
}
