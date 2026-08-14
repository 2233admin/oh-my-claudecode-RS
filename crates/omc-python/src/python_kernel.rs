//! Private Python subprocess and NDJSON runner.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::repl::{ExecuteResult, StateResult};
use crate::session::{MAX_TIMEOUT_MS, PythonSessionError};

pub(crate) struct PythonKernel {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<Result<Value, String>>,
    next_id: u64,
    project_dir: PathBuf,
}

impl PythonKernel {
    pub(crate) fn start(project_dir: &Path) -> Result<Self, String> {
        let command = std::env::var_os("OMC_PYTHON_COMMAND")
            .map(PathBuf::from)
            .or_else(|| find_python_command("python"))
            .or_else(|| find_python_command("python3"))
            .ok_or_else(|| "python or python3 was not found on PATH".to_string())?;
        let mut child = Command::new(command)
            .arg("-u")
            .arg("-c")
            .arg(PYTHON_RUNNER)
            .env("PYTHONIOENCODING", "utf-8")
            .current_dir(project_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "python stdin was unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "python stdout was unavailable".to_string())?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let item = match line {
                    Ok(line) => serde_json::from_str::<Value>(&line).map_err(|e| e.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                if sender.send(item).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            responses: receiver,
            next_id: 1,
            project_dir: project_dir.to_path_buf(),
        })
    }

    pub(crate) fn execute(
        &mut self,
        code: &str,
        timeout_ms: u64,
    ) -> Result<ExecuteResult, PythonSessionError> {
        let response = self.call("execute", Some(code), timeout_ms)?;
        serde_json::from_value(response).map_err(|error| {
            PythonSessionError::Failed(format!("invalid execute response: {error}"))
        })
    }

    pub(crate) fn state(&mut self) -> Result<StateResult, PythonSessionError> {
        let response = self.call("state", None, 30_000)?;
        serde_json::from_value(response)
            .map_err(|error| PythonSessionError::Failed(format!("invalid state response: {error}")))
    }

    fn call(
        &mut self,
        action: &str,
        code: Option<&str>,
        timeout_ms: u64,
    ) -> Result<Value, PythonSessionError> {
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(PythonSessionError::InvalidRequest(format!(
                "executionTimeout must be between 1 and {MAX_TIMEOUT_MS} milliseconds"
            )));
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let line = serde_json::to_string(&json!({"id": id, "action": action, "code": code}))
            .map_err(|error| PythonSessionError::Failed(error.to_string()))?;
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|error| PythonSessionError::Failed(error.to_string()))?;

        let response = match self
            .responses
            .recv_timeout(Duration::from_millis(timeout_ms))
        {
            Ok(response) => response.map_err(PythonSessionError::Failed)?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.restart().map_err(|error| {
                    PythonSessionError::Failed(format!(
                        "execution timed out and kernel restart failed: {error}"
                    ))
                })?;
                return Err(PythonSessionError::Timeout(timeout_ms));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(PythonSessionError::Failed(
                    "python process closed its output".into(),
                ));
            }
        };
        if response.get("id") != Some(&json!(id)) {
            return Err(PythonSessionError::Failed(
                "python response id did not match request".into(),
            ));
        }
        if response.get("ok") != Some(&Value::Bool(true)) {
            return Err(PythonSessionError::Failed(
                response
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("python execution failed")
                    .to_string(),
            ));
        }
        response
            .get("result")
            .cloned()
            .ok_or_else(|| PythonSessionError::Failed("python response omitted result".into()))
    }

    pub(crate) fn restart(&mut self) -> Result<(), PythonSessionError> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let replacement =
            Self::start(&self.project_dir).map_err(PythonSessionError::Unavailable)?;
        *self = replacement;
        Ok(())
    }
}

impl Drop for PythonKernel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn find_python_command(name: &str) -> Option<PathBuf> {
    let candidate = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&candidate))
        .find(|path| path.is_file())
        .or_else(|| Some(PathBuf::from(name)))
}

const PYTHON_RUNNER: &str = r#"
import contextlib
import datetime
import io
import json
import sys
import time
import traceback

MAX_OUTPUT = 4 * 1024 * 1024
namespace = {"__name__": "__main__"}

class LimitedWriter(io.TextIOBase):
    def __init__(self):
        self.parts = []
        self.size = 0
        self.truncated = False

    def write(self, value):
        if not isinstance(value, str):
            value = str(value)
        remaining = MAX_OUTPUT - self.size
        if remaining <= 0:
            self.truncated = True
            return len(value)
        encoded = value.encode("utf-8")
        piece = encoded[:remaining].decode("utf-8", errors="ignore")
        piece_size = len(piece.encode("utf-8"))
        self.parts.append(piece)
        self.size += piece_size
        if piece_size != len(encoded):
            self.truncated = True
        return len(value)

    def flush(self):
        return None

    def text(self):
        return "".join(self.parts)

def memory():
    return {"rss_mb": 0.0, "vms_mb": 0.0}

def variables():
    return sorted(name for name in namespace if not name.startswith("__"))

def response(req):
    action = req.get("action")
    if action == "state":
        return {"ok": True, "result": {"memory": memory(), "variables": variables(), "variable_count": len(variables())}}
    if action != "execute":
        return {"ok": False, "error": "unsupported action"}
    code = req.get("code")
    if not isinstance(code, str) or not code:
        return {"ok": False, "error": "code is required"}
    stdout = LimitedWriter()
    stderr = LimitedWriter()
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    begin = time.monotonic()
    error = None
    success = True
    try:
        original_stdin = sys.stdin
        sys.stdin = io.StringIO()
        try:
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                exec(compile(code, "<omc-python>", "exec"), namespace, namespace)
        finally:
            sys.stdin = original_stdin
    except BaseException as exc:
        success = False
        error = {"type": type(exc).__name__, "message": str(exc), "traceback": traceback.format_exc()}
    markers = []
    if stdout.truncated:
        markers.append({"type": "output", "subtype": "stdout_truncated", "content": "stdout exceeded 4 MiB", "line_number": 0, "category": "limit"})
    if stderr.truncated:
        markers.append({"type": "output", "subtype": "stderr_truncated", "content": "stderr exceeded 4 MiB", "line_number": 0, "category": "limit"})
    return {"ok": True, "result": {
        "success": success,
        "stdout": stdout.text(),
        "stderr": stderr.text(),
        "markers": markers,
        "timing": {"started_at": started, "duration_ms": int((time.monotonic() - begin) * 1000)},
        "memory": memory(),
        "error": error,
    }}

for line in sys.stdin:
    try:
        req = json.loads(line)
        result = response(req)
        print(json.dumps({"id": req.get("id"), **result}, ensure_ascii=False, separators=(",", ":")), flush=True)
    except BaseException as exc:
        print(json.dumps({"id": None, "ok": False, "error": f"runner error: {exc}"}, separators=(",", ":")), flush=True)
"#;
