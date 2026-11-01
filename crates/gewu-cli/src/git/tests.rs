use std::{fs, path::Path, process::Command};

use gewu_review::SourceIdentity;

use super::{DiffParser, Repository};

/// Wide enough that these fixtures are not split by the budget.
const BUDGET: usize = 200;

#[test]
fn parses_hunks_into_stable_changes() {
    let diff = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1,2 @@\n-a\n+b\n+c\n";
    let source = SourceIdentity::new("base", "head", diff.as_bytes());
    let input = DiffParser::new(source, diff, BUDGET).parse();

    assert_eq!(input.changes.len(), 1);
    assert_eq!(input.changes[0].path, "a.rs");
    assert_eq!(
        (input.changes[0].old_lines, input.changes[0].new_lines),
        (1, 2)
    );
}

#[test]
fn attributes_each_hunk_to_its_own_file() {
    let diff = concat!(
        "diff --git a/kept.rs b/kept.rs\n--- a/kept.rs\n+++ b/kept.rs\n@@ -1 +1 @@\n-a\n+b\n",
        "diff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\nindex 1234567..0000000\n",
        "--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n",
        "diff --git a/also-gone.rs b/also-gone.rs\ndeleted file mode 100644\n",
        "--- a/also-gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-also gone\n",
    );
    let source = SourceIdentity::new("base", "head", diff.as_bytes());
    let input = DiffParser::new(source, diff, BUDGET).parse();

    let paths: Vec<_> = input
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    assert_eq!(paths, vec!["kept.rs", "gone.rs", "also-gone.rs"]);
}

#[test]
fn records_changes_that_carry_no_hunk() {
    let diff = concat!(
        "diff --git a/logo.png b/logo.png\nnew file mode 100644\nindex 0000000..1234567\n",
        "Binary files /dev/null and b/logo.png differ\n",
        "diff --git a/run.sh b/run.sh\nold mode 100644\nnew mode 100755\n",
        "diff --git a/old.rs b/new.rs\nsimilarity index 100%\nrename from old.rs\nrename to new.rs\n",
        "diff --git a/empty.txt b/empty.txt\nnew file mode 100644\nindex 0000000..e69de29\n",
    );
    let source = SourceIdentity::new("base", "head", diff.as_bytes());
    let input = DiffParser::new(source, diff, BUDGET).parse();

    let recorded: Vec<_> = input
        .changes
        .iter()
        .map(|change| (change.path.as_str(), change.patch.trim()))
        .collect();
    assert_eq!(
        recorded,
        vec![
            (
                "logo.png",
                "new file mode 100644\nBinary files /dev/null and b/logo.png differ"
            ),
            ("run.sh", "old mode 100644\nnew mode 100755"),
            (
                "new.rs",
                "similarity index 100%\nrename from old.rs\nrename to new.rs"
            ),
            ("empty.txt", "new file mode 100644"),
        ]
    );
}

#[test]
fn records_a_rename_alongside_the_hunks_that_follow_it() {
    let diff = concat!(
        "diff --git a/old.rs b/new.rs\nsimilarity index 80%\nrename from old.rs\nrename to new.rs\n",
        "--- a/old.rs\n+++ b/new.rs\n@@ -1 +1 @@\n-a\n+b\n",
    );
    let source = SourceIdentity::new("base", "head", diff.as_bytes());
    let input = DiffParser::new(source, diff, BUDGET).parse();

    assert_eq!(input.changes.len(), 2);
    assert!(input.changes[0].patch.contains("rename from old.rs"));
    assert!(input.changes[1].patch.starts_with("@@ "));
    assert!(input.changes.iter().all(|change| change.path == "new.rs"));
}

#[test]
fn leaves_ordinary_edits_without_a_metadata_unit() {
    let diff = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-a\n+b\n";
    let source = SourceIdentity::new("base", "head", diff.as_bytes());
    let input = DiffParser::new(source, diff, BUDGET).parse();

    assert_eq!(input.changes.len(), 1);
    assert!(input.changes[0].patch.starts_with("@@ "));
}

#[test]
fn a_hunk_keeps_its_id_when_an_unrelated_edit_shifts_it() {
    let before =
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -10,1 +10,2 @@\n context\n+added\n";
    let after = concat!(
        // An edit earlier in the file pushes the same hunk five lines down.
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,0 +1,5 @@\n+earlier\n+earlier\n+earlier\n+earlier\n+earlier\n",
        "@@ -10,1 +15,2 @@\n context\n+added\n",
    );

    let first = DiffParser::new(
        SourceIdentity::new("base", "head-1", before.as_bytes()),
        before,
        BUDGET,
    )
    .parse();
    let second = DiffParser::new(
        SourceIdentity::new("base", "head-2", after.as_bytes()),
        after,
        BUDGET,
    )
    .parse();

    let shifted = second
        .changes
        .iter()
        .find(|change| change.patch.contains("+added"))
        .expect("shifted hunk");
    assert_eq!(shifted.id, first.changes[0].id);
    assert_eq!(shifted.new_start, 15);
    assert_eq!(
        second.unchanged_since(&first).len(),
        1,
        "only the shifted hunk carries over"
    );
}

#[test]
fn identical_hunks_in_one_file_stay_distinct() {
    let diff = concat!(
        "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n",
        "@@ -1,0 +1,1 @@\n+same\n",
        "@@ -9,0 +9,1 @@\n+same\n",
    );

    let input = DiffParser::new(
        SourceIdentity::new("base", "head", diff.as_bytes()),
        diff,
        BUDGET,
    )
    .parse();

    assert_eq!(input.changes.len(), 2);
    assert_ne!(input.changes[0].id, input.changes[1].id);
}

#[test]
fn captures_worktree_without_changing_real_index() {
    let directory = tempfile::tempdir().expect("temporary repository");
    git(directory.path(), &["init"]);
    fs::write(directory.path().join("tracked.rs"), "fn before() {}\n").expect("write tracked file");
    git(directory.path(), &["add", "."]);
    commit(directory.path(), "initial");
    let index_path = directory.path().join(".git/index");
    let index_before = fs::read(&index_path).expect("read index");
    fs::write(directory.path().join("tracked.rs"), "fn after() {}\n").expect("modify tracked file");
    fs::write(directory.path().join("untracked.rs"), "fn added() {}\n")
        .expect("write untracked file");

    let input = Repository::new(directory.path())
        .worktree_input("HEAD", BUDGET)
        .expect("capture worktree");

    let paths: Vec<_> = input
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    assert!(paths.contains(&"tracked.rs"));
    assert!(paths.contains(&"untracked.rs"));
    assert_eq!(fs::read(index_path).expect("reread index"), index_before);
}

#[test]
fn keeps_gewu_state_out_of_the_review_whether_or_not_it_is_ignored() {
    for ignored in [false, true] {
        let directory = tempfile::tempdir().expect("temporary repository");
        let root = directory.path();
        git(root, &["init"]);
        fs::write(root.join("a.rs"), "fn before() {}\n").expect("write source");
        if ignored {
            fs::write(root.join(".gitignore"), "/.gewu\n").expect("write gitignore");
        }
        git(root, &["add", "-A"]);
        commit(root, "initial");
        fs::write(root.join("a.rs"), "fn after() {}\n").expect("edit source");
        fs::create_dir_all(root.join(".gewu/harness")).expect("create state directory");
        fs::write(root.join(".gewu/harness/state.json"), "{}\n").expect("write state");

        let input = Repository::new(root)
            .worktree_input("HEAD", BUDGET)
            .expect("capture worktree");

        let paths: Vec<_> = input
            .changes
            .iter()
            .map(|change| change.path.as_str())
            .collect();
        assert_eq!(paths, vec!["a.rs"], "ignored={ignored}");
    }
}

#[test]
fn captures_deletions_and_binary_files_from_a_real_repository() {
    let directory = tempfile::tempdir().expect("temporary repository");
    git(directory.path(), &["init"]);
    fs::write(directory.path().join("kept.rs"), "fn kept() {}\n").expect("write kept file");
    fs::write(directory.path().join("gone.rs"), "fn gone() {}\n").expect("write doomed file");
    git(directory.path(), &["add", "."]);
    commit(directory.path(), "initial");
    fs::write(directory.path().join("kept.rs"), "fn edited() {}\n").expect("edit kept file");
    fs::remove_file(directory.path().join("gone.rs")).expect("delete file");
    fs::write(directory.path().join("logo.png"), [0u8, 1, 2, 0, 255, 0])
        .expect("write binary file");

    let input = Repository::new(directory.path())
        .worktree_input("HEAD", BUDGET)
        .expect("capture worktree");

    let paths: Vec<_> = input
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    assert!(paths.contains(&"kept.rs"), "{paths:?}");
    assert!(paths.contains(&"gone.rs"), "{paths:?}");
    assert!(paths.contains(&"logo.png"), "{paths:?}");
    let binary = input
        .changes
        .iter()
        .find(|change| change.path == "logo.png")
        .expect("binary change");
    assert!(binary.patch.contains("Binary files"), "{}", binary.patch);
    let deleted = input
        .changes
        .iter()
        .find(|change| change.path == "gone.rs")
        .expect("deleted change");
    assert!(deleted.patch.contains("-fn gone() {}"), "{}", deleted.patch);
}

fn commit(root: &Path, message: &str) {
    git(
        root,
        &[
            "-c",
            "user.name=Gewu Test",
            "-c",
            "user.email=gewu@example.test",
            "commit",
            "-m",
            message,
        ],
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The audit compares the manifest against an independent listing of the same
/// diff, so the two must agree on deletions, binaries, and worktree trees.
#[test]
fn audit_scope_and_symbols_read_the_reviewed_tree() {
    let directory = tempfile::tempdir().expect("temporary repository");
    git(directory.path(), &["init"]);
    fs::create_dir(directory.path().join("src")).expect("create source directory");
    fs::write(directory.path().join("src/kept.rs"), "fn kept() {}\n").expect("write kept file");
    fs::write(directory.path().join("gone.rs"), "fn gone() {}\n").expect("write doomed file");
    git(directory.path(), &["add", "."]);
    commit(directory.path(), "initial");
    fs::write(directory.path().join("src/kept.rs"), "fn edited() {}\n").expect("edit kept file");
    fs::remove_file(directory.path().join("gone.rs")).expect("delete file");
    fs::write(directory.path().join("logo.png"), [0u8, 1, 2, 0, 255, 0])
        .expect("write binary file");
    let repository = Repository::new(directory.path());
    let input = repository
        .worktree_input("HEAD", BUDGET)
        .expect("capture worktree");
    let (base, head) = (&input.source.base, &input.source.head);

    let mut scope = repository
        .changed_paths(base, head, &[])
        .expect("list changed paths");
    scope.sort();
    let mut manifest: Vec<_> = input.changes.iter().map(|c| c.path.clone()).collect();
    manifest.sort();
    assert_eq!(scope, manifest);
    assert_eq!(
        repository
            .changed_paths(base, head, &["src".into()])
            .expect("list scoped paths"),
        ["src/kept.rs"]
    );

    let words = repository.tree_words(head).expect("read tree words");
    assert!(words.iter().any(|word| word == "edited"), "{words:?}");
    assert!(!words.iter().any(|word| word == "gone"), "{words:?}");
    assert_eq!(
        repository.tree_paths(head).expect("list tree paths"),
        ["logo.png", "src/kept.rs"]
    );
}
