//! Saying a line to the terminal that started the process (`--version`).
//!
//! The Windows release is a GUI program with no console of its own: `println!` reaches nobody there
//! unless the shell redirected the output, so `say` borrows the parent's console in that case. A
//! shell does not wait for a GUI program, so cmd and PowerShell show the line after the prompt came
//! back; a pipe (`takeshot --version | more`) or `start /wait` keeps them in order.
//!
//! `cargo test` cannot reach the borrowed console (the debug build is a console program with its
//! own standard output): `scripts/windows-version-check.ps1` runs the release exe in a real one.

use std::io::Write;

/// Writes `line` and a newline to standard output, or to the parent's console when the process has
/// no standard output (Windows release, started from a shell with nothing redirected). A closed
/// pipe (`| head -0`) is no error worth a panic.
pub fn say(line: &str) {
    #[cfg(windows)]
    if !has_stdout() {
        return say_to_parent_console(line);
    }
    let _ = writeln!(std::io::stdout(), "{line}");
}

/// Whether the process came with a standard output (a console, a file or a pipe).
#[cfg(windows)]
fn has_stdout() -> bool {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_OUTPUT_HANDLE};

    // SAFETY: GetStdHandle takes a constant and reads the process's own handle table.
    let out = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    !out.is_null() && out != INVALID_HANDLE_VALUE
}

/// Attaches to the console of the shell that started us and writes there; with no shell (started
/// from the Start menu or Explorer) there is nobody to tell.
#[cfg(windows)]
fn say_to_parent_console(line: &str) {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

    // SAFETY: AttachConsole takes a process id constant and changes only this process's console.
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
        return;
    }
    if let Ok(mut console) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
        let _ = writeln!(console, "{line}");
    }
}
