//! Two rules of the release's supply chain that nothing in the app would notice breaking:
//!
//! - every action the workflows use is pinned by the commit of a release, with its version in a
//!   comment, so a moved tag cannot change what runs with the release's permissions;
//! - every build, lint and test in CI and in the release scripts runs with `--locked`, so a
//!   `Cargo.lock` out of step with `Cargo.toml` fails instead of quietly resolving newer crates.

use std::fs;
use std::path::PathBuf;

/// The scripts that build or check what ships (the dev loop and the contributor's own commands are
/// free to update the lock).
const SCRIPTS: [&str; 3] = [
    "scripts/check.sh",
    "scripts/linux-packages.sh",
    "scripts/release-local.sh",
];

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn workflows() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("the workflows folder")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(".yml"))
        .map(|n| format!(".github/workflows/{n}"))
        .collect();
    names.sort();
    names
}

#[test]
fn every_action_is_pinned_by_the_commit_of_a_release() {
    let mut pinned = 0;
    for file in workflows() {
        for (n, line) in read(&file).lines().enumerate() {
            let line = line.trim().trim_start_matches("- ");
            let Some(used) = line.strip_prefix("uses:") else {
                continue;
            };
            let (spec, version) = used.split_once('#').unwrap_or((used, ""));
            let spec = spec.trim();
            // A workflow of this repository is called by path.
            if spec.starts_with("./") {
                continue;
            }
            let at = format!("{file}:{}", n + 1);
            let (_, commit) = spec
                .split_once('@')
                .unwrap_or_else(|| panic!("{at}: no @ in {spec}"));
            let is_commit = commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit());
            assert!(is_commit, "{at}: {spec} is not pinned by a commit");
            assert!(
                version.trim().starts_with('v'),
                "{at}: say the version in a comment"
            );
            pinned += 1;
        }
    }
    assert!(
        pinned >= 8,
        "found only {pinned} pinned actions: did the workflows move?"
    );
}

#[test]
fn every_build_lint_and_test_of_the_release_path_is_locked() {
    let mut files = workflows();
    files.extend(SCRIPTS.map(String::from));
    let mut calls = 0;
    for file in files {
        for (n, line) in read(&file).lines().enumerate() {
            let line = line.trim();
            // Comments, and the text a script prints about a step, only name the commands.
            if line.starts_with('#') || line.starts_with("step ") || line.starts_with("die ") {
                continue;
            }
            let runs_cargo = ["build", "clippy", "test", "install"]
                .iter()
                .any(|verb| line.contains(&format!("cargo {verb}")));
            if runs_cargo {
                assert!(
                    line.contains("--locked"),
                    "{file}:{}: `{line}` has no --locked",
                    n + 1
                );
                calls += 1;
            }
        }
    }
    assert!(
        calls >= 10,
        "found only {calls} cargo calls: did the files move?"
    );
}
