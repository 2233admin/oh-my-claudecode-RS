use std::collections::HashMap;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HookSpecificOutput {
    pub hook_event_name: String,
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HookResult {
    pub continue_: bool,
    pub suppress_output: bool,
    pub hook_specific_output: Option<HookSpecificOutput>,
    pub error: Option<String>,
}

impl Default for HookResult {
    fn default() -> Self {
        Self {
            continue_: true,
            suppress_output: false,
            hook_specific_output: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookExecutor {
    default_timeout: Duration,
    cwd: Option<PathBuf>,
    env: HashMap<String, String>,
}

impl HookExecutor {
    pub fn new() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            cwd: None,
            env: HashMap::new(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_envs(mut self, env: HashMap<String, String>) -> Self {
        for (key, value) in env {
            if let Some(existing) = self.env.get_mut(&key) {
                *existing += &value;
            } else {
                self.env.insert(key, value);
            }
        }
        self
    }

    pub fn execute(&self, command: &str, input: Option<&str>) -> HookResult {
        let mut parts = command.split_whitespace();
        let program = match parts.next() {
            Some(p) => p,
            None => {
                return HookResult {
                    continue_: true,
                    error: Some("Empty command".to_string()),
                    ..Default::default()
                };
            }
        };
        let args: Vec<&str> = parts.collect();

        let mut cmd = Command::new(program);
        cmd.args(&args);

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        if let Some(_input) = input {
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return HookResult {
                    continue_: true,
                    error: Some(format!("Failed to spawn command '{program}': {e}")),
                    ..Default::default()
                };
            }
        };

        // Write input if provided
        if let Some(input_data) = input
            && let Some(ref mut stdin) = child.stdin
        {
            let _ = stdin.write_all(input_data.as_bytes());
        }

        // Wait for the child with timeout
        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                return HookResult {
                    continue_: true,
                    error: Some(format!("Command execution failed: {e}")),
                    ..Default::default()
                };
            }
        };

        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if success {
            HookResult {
                continue_: true,
                suppress_output: false,
                hook_specific_output: Some(HookSpecificOutput {
                    hook_event_name: program.to_string(),
                    additional_context: Some(stdout),
                }),
                error: None,
            }
        } else {
            let error_msg = if stderr.is_empty() {
                format!("Command failed with exit code: {:?}", output.status.code())
            } else {
                stderr.to_string()
            };
            HookResult {
                continue_: true,
                suppress_output: false,
                hook_specific_output: None,
                error: Some(error_msg),
            }
        }
    }

    pub fn execute_chain(&self, commands: &[&str], input: Option<&str>) -> HookResult {
        let mut current_input = input.map(String::from);
        let mut last_error: Option<String> = None;
        let mut hook_specific_output: Option<HookSpecificOutput> = None;

        for (i, command) in commands.iter().enumerate() {
            let result = self.execute(command, current_input.as_deref());

            if let Some(ref err) = result.error {
                last_error = Some(format!("Command {i} failed: {err}"));
            }

            if let Some(ref output) = result.hook_specific_output {
                hook_specific_output = Some(output.clone());
                if let Some(ref ctx) = output.additional_context {
                    current_input = Some(ctx.clone());
                }
            }

            if result.error.is_some() {
                break;
            }
        }

        if let Some(error) = last_error {
            HookResult {
                continue_: true,
                suppress_output: false,
                hook_specific_output: None,
                error: Some(error),
            }
        } else {
            HookResult {
                continue_: true,
                suppress_output: false,
                hook_specific_output: hook_specific_output.or_else(|| {
                    Some(HookSpecificOutput {
                        hook_event_name: "chain".to_string(),
                        additional_context: current_input,
                    })
                }),
                error: None,
            }
        }
    }
}

impl Default for HookExecutor {
    fn default() -> Self {
        Self {
            stdin: Stdio::inherit(),
            stdout: Stdio::inherit(),
            stderr: Stdio::inherit(),
            hooks: Vec::new(),
            override_exit_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_executor_default_creation() {
        let executor = HookExecutor::default();
        assert_eq!(executor.default_timeout, Duration::from_secs(30));
        assert!(executor.cwd.is_none());
        assert!(executor.env.is_empty());
    }

    #[test]
    fn hook_executor_with_timeout() {
        let executor = HookExecutor::default().with_timeout(Duration::from_secs(60));
        assert_eq!(executor.default_timeout, Duration::from_secs(60));
    }

    #[test]
    fn hook_executor_with_cwd() {
        let temp_dir = tempdir().unwrap();
        let executor = HookExecutor::default().with_cwd(temp_dir.path().to_path_buf());
        assert_eq!(executor.cwd, Some(temp_dir.path().to_path_buf()));
    }

    #[test]
    fn hook_executor_with_env() {
        let executor = HookExecutor::default().with_env("KEY", "VALUE");
        assert_eq!(executor.env.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn hook_executor_with_envs() {
        let mut env = HashMap::default();
        env.insert("A".to_string(), "1".to_string());
        env.insert("B".to_string(), "2".to_string());
        let executor = HookExecutor::default().with_envs(env);
        assert_eq!(executor.env.get("A"), Some(&"1".to_string()));
        assert_eq!(executor.env.get("B"), Some(&"2".to_string()));
    }

    #[test]
    fn hook_result_default() {
        let result = HookResult::default();
        assert!(result.continue_);
        assert!(!result.suppress_output);
        assert!(result.hook_specific_output.is_none());
        assert!(result.error.is_none());
    }

    #[test]
    fn hook_specific_output_creation() {
        let output = HookSpecificOutput {
            hook_event_name: "test".to_string(),
            additional_context: Some("context".to_string()),
        };
        assert_eq!(output.hook_event_name, "test");
        assert_eq!(output.additional_context, Some("context".to_string()));
    }

    #[test]
    fn execute_echo_command() {
        let executor = HookExecutor::default();
        let result = executor.execute("echo hello", None);
        assert!(result.error.is_none());
        assert!(result.hook_specific_output.is_some());
        if let Some(output) = result.hook_specific_output {
            assert!(output.additional_context.unwrap().contains("hello"));
        }
    }

    #[test]
    fn execute_with_input() {
        let executor = HookExecutor::default();
        let result = executor.execute("cat", Some("test input"));
        assert!(result.error.is_none());
        if let Some(output) = result.hook_specific_output {
            assert!(output.additional_context.unwrap().contains("test input"));
        }
    }

    #[test]
    fn execute_chain() {
        let executor = HookExecutor::default();
        let commands = vec!["echo hello", "cat"];
        let result = executor.execute_chain(&commands, None);
        assert!(result.error.is_none());
        assert!(result.hook_specific_output.is_some());
    }

    #[test]
    fn execute_empty_command() {
        let executor = HookExecutor::default();
        let result = executor.execute("", None);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Empty command"));
    }

    #[test]
    fn execute_nonexistent_command() {
        let executor = HookExecutor::default();
        let result = executor.execute("nonexistent_command_12345", None);
        assert!(result.error.is_some());
    }
}
