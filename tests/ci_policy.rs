//! Four rules of the release's supply chain that nothing in the app would notice breaking:
//!
//! - every action the workflows use is pinned by the commit of a release, with its version in a
//!   comment, so a moved tag cannot change what runs with the release's permissions;
//! - every build, lint and test in CI and in the release scripts runs with `--locked`, so a
//!   `Cargo.lock` out of step with `Cargo.toml` fails instead of quietly resolving newer crates;
//! - CI and the Linux container build with one exact Rust version, named the same in every place
//!   that runs cargo, so a new Rust cannot break the lint with no change of ours;
//! - the Linux job of CI and the image of `scripts/ci-linux` (which builds the release's Linux files)
//!   are the same distribution, whose glibc is the floor of every Linux download.

use std::fs;
use std::path::PathBuf;

/// The scripts that build or check what ships (the dev loop and the contributor's own commands are
/// free to update the lock).
const SCRIPTS: [&str; 3] = [
    "scripts/check.sh",
    "scripts/linux-packages.sh",
    "scripts/release-local.sh",
];

/// Whether a line runs one of the cargo subcommands that resolve dependencies: the word `cargo`
/// (or `cargo.exe`), an optional `+toolchain`, then the subcommand, whatever the spacing.
fn runs_cargo(line: &str) -> bool {
    const RESOLVING: [&str; 9] = [
        "build", "check", "clippy", "test", "install", "run", "doc", "bench", "fetch",
    ];
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        if word == "cargo" || word == "cargo.exe" {
            let next = words.clone().find(|w| !w.starts_with('+'));
            if next.is_some_and(|sub| RESOLVING.contains(&sub)) {
                return true;
            }
        }
    }
    false
}

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
        .filter(|n| n.ends_with(".yml") || n.ends_with(".yaml"))
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
            let line = line.trim().trim_start_matches(['-', ' ']);
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
            if runs_cargo(line) {
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

#[test]
fn the_cargo_calls_are_found_whatever_the_spacing() {
    for yes in [
        "cargo build --release",
        "cargo  test",
        "cargo +nightly clippy --all-targets",
        "cargo.exe check",
        r#"echo "== linux: clippy"; cargo clippy --all-targets"#,
        "run windows-build cargo build --release",
    ] {
        assert!(runs_cargo(yes), "{yes}");
    }
    for no in [
        "cargo fmt --check",
        "cargo packager --release --formats deb",
        "command -v cargo-packager > /dev/null",
        "cargo-packager --version",
        "export CARGO_HOME=/opt/cargo",
        "- name: Lint and test",
    ] {
        assert!(!runs_cargo(no), "{no}");
    }
}

/// The first word after `key` on the first line of `file` that starts with it.
fn version_after(file: &str, key: &str) -> String {
    let text = read(file);
    let line = text.lines().map(str::trim).find(|l| l.starts_with(key));
    let line = line.unwrap_or_else(|| panic!("{file}: no line starts with `{key}`"));
    let word = line[key.len()..].split_whitespace().next();
    word.unwrap_or_else(|| panic!("{file}: nothing after `{key}`"))
        .to_owned()
}

#[test]
fn the_rust_toolchain_is_pinned_to_one_version_in_every_place() {
    // A developer's `rust-toolchain.toml` follows stable (a host cannot go with an exact version).
    let places = [
        version_after(".github/workflows/build.yml", "RUSTUP_TOOLCHAIN:"),
        version_after("scripts/ci-linux/Dockerfile", "RUSTUP_TOOLCHAIN="),
        version_after("scripts/ci-linux/Dockerfile", "--default-toolchain"),
    ];
    let version = &places[0];
    let numbers: Vec<_> = version.split('.').collect();
    assert!(
        numbers.len() == 3 && numbers.iter().all(|n| n.parse::<u32>().is_ok()),
        "`{version}` is not an exact version: `stable` would move under us"
    );
    assert!(
        places.iter().all(|p| p == version),
        "the pins differ: {places:?}"
    );
}

/// A workflow that runs cargo has no `rust-toolchain.toml` it can use (its host is Windows' and
/// rustup refuses it elsewhere), so it names the pinned version itself, the one `build.yml` has.
#[test]
fn every_workflow_that_runs_cargo_names_the_pinned_toolchain() {
    let pinned = version_after(".github/workflows/build.yml", "RUSTUP_TOOLCHAIN:");
    let mut checked = 0;
    for file in workflows() {
        let runs = read(&file)
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .any(runs_cargo);
        if runs {
            assert_eq!(version_after(&file, "RUSTUP_TOOLCHAIN:"), pinned, "{file}");
            checked += 1;
        }
    }
    // build.yml and audit.yml; the others only call build.yml.
    assert!(
        checked >= 2,
        "found only {checked} workflows that run cargo"
    );
}

/// The glibc a Linux binary needs is the one of the machine that linked it, so the job that builds
/// the files CI offers and the image that builds the release's must be the same distribution.
#[test]
fn the_linux_job_and_the_linux_image_are_the_same_distribution() {
    let image = version_after("scripts/ci-linux/Dockerfile", "FROM");
    let workflow = read(".github/workflows/build.yml");
    let key = "\"container\":\"";
    let start = workflow
        .find(key)
        .unwrap_or_else(|| panic!("build.yml: the Linux target has no {key}"))
        + key.len();
    let container = workflow[start..].split('"').next().expect("a value");
    assert_eq!(
        container, image,
        "CI and scripts/ci-linux build on different distributions"
    );
}

/// What the Linux job caches (the Rust build and `cargo-packager`) is keyed by its container, so a
/// `target` built on Ubuntu or a `cargo-packager` linked against a newer glibc is never restored
/// into the Debian job.
#[test]
fn the_linux_job_runs_in_its_container_and_caches_by_it() {
    let workflow = read(".github/workflows/build.yml");
    let lines: Vec<&str> = workflow.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"container: ${{ matrix.container }}"),
        "build.yml: the job does not use the container of its target"
    );
    assert!(
        lines.contains(&"key: ${{ matrix.container }}"),
        "build.yml: the Rust cache is not keyed by the container"
    );
    let packager = lines
        .iter()
        .find(|l| l.starts_with("key: cargo-packager-"))
        .expect("build.yml: the cargo-packager cache key");
    assert!(
        packager.contains("matrix.container"),
        "build.yml: the cargo-packager cache is not keyed by the container: {packager}"
    );
}
