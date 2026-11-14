//! `takeshot --version` is a verb: it says the version on standard output and leaves, with no window
//! and no resident, so a script (a packager, a bug report) can read it.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Longer than any start of the app we would want to wait for: a `--version` that opens the main
/// window instead never leaves, and the test must not hang with it.
const LIMIT: Duration = Duration::from_secs(10);

#[test]
fn version_prints_the_name_and_the_version_and_leaves() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_takeshot"))
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start takeshot");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait for takeshot") {
            break status;
        }
        if started.elapsed() > LIMIT {
            child.kill().expect("stop takeshot");
            child.wait().expect("reap takeshot");
            panic!("`takeshot --version` did not leave: it started the app");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut out = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut out)
        .expect("read stdout");
    assert!(status.success(), "exit {status}");
    assert_eq!(out, format!("takeshot {}\n", env!("CARGO_PKG_VERSION")));
}
