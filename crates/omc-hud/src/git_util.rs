use std::process::{Command, Stdio};
use std::time::Duration;

/// Run a git command with a timeout. Returns stdout bytes on success, None on timeout or error.
pub fn git_output_timeout(args: &[&str], timeout: Duration) -> Option<std::process::Output> {
    use std::sync::mpsc;
    use std::thread;

    let mut cmd = Command::new("git");
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().ok()?;
    let stdout = child.stdout.take()?;

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        let mut r = std::io::BufReader::new(stdout);
        r.read_to_end(&mut buf).ok();
        let _ = tx.send(buf);
    });

    match rx.recv_timeout(timeout) {
        Ok(stdout_bytes) => {
            let status = child.wait().ok()?;
            Some(std::process::Output {
                status,
                stdout: stdout_bytes,
                stderr: Vec::new(),
            })
        }
        Err(_) => {
            // Timed out — kill the process
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}
