//! The shell scripts of the repo parse. Nothing in `cargo test` runs them, and a quote broken
//! inside a `bash -c "..."` string shows only when someone runs the script: the Linux packages of a
//! release did not build for a line that echoed the compiler's version.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The scripts, as `/` paths from the repo's root: every `*.sh` under `scripts/` and every file in
/// `.githooks/` (hooks have no extension).
fn scripts() -> Vec<String> {
    fn walk(dir: &str, hooks: bool, out: &mut Vec<String>) {
        let mut entries: Vec<_> = fs::read_dir(root().join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("an entry"))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let relative = format!("{dir}/{name}");
            if e.file_type().expect("a file type").is_dir() {
                walk(&relative, hooks, out);
            } else if hooks || name.ends_with(".sh") {
                out.push(relative);
            }
        }
    }
    let mut out = Vec::new();
    walk("scripts", false, &mut out);
    walk(".githooks", true, &mut out);
    out
}

/// The GNU bash of this machine, if it has one. On Windows `bash` on the PATH is often the
/// stand-in that starts a WSL distribution nobody installed, so Git's own is tried too.
fn gnu_bash() -> Option<String> {
    let mut candidates = vec!["bash".to_string()];
    if let Some(files) = std::env::var_os("ProgramFiles") {
        let git = PathBuf::from(files).join("Git/usr/bin/bash.exe");
        candidates.push(git.to_string_lossy().into_owned());
    }
    candidates.into_iter().find(|bash| {
        Command::new(bash).arg("--version").output().is_ok_and(|o| {
            o.status.success() && String::from_utf8_lossy(&o.stdout).contains("GNU bash")
        })
    })
}

#[test]
fn every_shell_script_parses() {
    let found = scripts();
    assert!(found.len() >= 10, "found {found:?}");
    let Some(bash) = gnu_bash() else {
        if !cfg!(windows) {
            panic!("bash is missing: the scripts are not checked");
        }
        eprintln!("no GNU bash here: the scripts are checked on Linux and macOS");
        return;
    };
    for script in &found {
        let out = Command::new(&bash)
            .current_dir(root())
            .arg("-n")
            .arg(script)
            .output()
            .expect("bash runs");
        assert!(
            out.status.success(),
            "{script} does not parse: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
