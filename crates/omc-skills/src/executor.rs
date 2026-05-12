//! Skill executor module - runs skills with templating and state management

use std::collections::HashMap;
use thiserror::Error;

use crate::loader::SkillLoader;
use crate::state::SkillStateStore;

/// Errors that can occur during skill execution
#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Skill not found: {0}")]
    SkillNotFound(String),
    #[error("Invalid template: {0}")]
    TemplateError(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

/// Execution result from running a skill
#[derive(Debug)]
pub struct ExecutionResult {
    /// The skill name that was executed
    pub skill_name: String,
    /// The formatted output after template substitution
    pub output: String,
    /// Variables that were set during execution
    pub variables_set: Vec<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Skill executor that handles template rendering and execution
#[derive(Debug)]
pub struct SkillExecutor {
    loader: SkillLoader,
    state: SkillStateStore,
}

impl SkillExecutor {
    /// Create a new skill executor
    pub fn new(loader: SkillLoader) -> Self {
        Self {
            loader,
            state: SkillStateStore::new(),
        }
    }

    /// Execute a skill by name with provided arguments
    ///
    /// # Arguments
    ///
    /// * `name` - Skill name or alias
    /// * `args` - Key-value arguments for template substitution
    /// * `context` - Additional context for execution
    ///
    /// # Returns
    ///
    /// * `Ok(ExecutionResult)` if execution succeeds
    /// * `Err(ExecutorError)` if execution fails
    pub fn execute(
        &mut self,
        name: &str,
        args: HashMap<String, String>,
        context: Option<String>,
    ) -> Result<ExecutionResult, ExecutorError> {
        let start = std::time::Instant::now();

        // Load skill
        let skill = self
            .loader
            .load(name)
            .map_err(|_| ExecutorError::SkillNotFound(name.to_string()))?;

        // Collect keys before moving args
        let variables_set: Vec<String> = args.keys().cloned().collect();

        // Merge args into state
        for (key, value) in args {
            self.state.set(&key, value);
        }

        // Apply context if provided
        if let Some(ctx) = context {
            self.state.set("context", ctx);
        }

        // Format template with current state
        let output = self.format_template(&skill.content)?;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            skill_name: skill.metadata.name,
            output,
            variables_set,
            execution_time_ms,
        })
    }

    /// Format a template string by substituting variables
    ///
    /// Variables are specified as `{{variable_name}}` and are replaced
    /// with values from the state store.
    ///
    /// # Arguments
    ///
    /// * `template` - The template string with `{{variable}}` placeholders
    ///
    /// # Returns
    ///
    /// * `Ok(String)` with variables substituted
    /// * `Err(ExecutorError)` if template is invalid
    pub fn format_template(&self, template: &str) -> Result<String, ExecutorError> {
        let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();

        let result = re.replace_all(template, |caps: &regex::Captures| {
            let var_name = &caps[1];
            self.state.get(var_name).unwrap_or_default()
        });

        Ok(result.to_string())
    }

    /// Get a copy of the current state
    pub fn get_state(&self) -> SkillStateStore {
        self.state.clone()
    }

    /// Set a state variable directly
    pub fn set_variable(&mut self, key: &str, value: String) {
        self.state.set(key, value);
    }

    /// Get a state variable
    pub fn get_variable(&self, key: &str) -> Option<String> {
        self.state.get(key)
    }

    /// Clear all state
    pub fn clear_state(&mut self) {
        self.state.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn _create_test_executor() -> (SkillExecutor, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        fs::write(
            skills_dir.join("test-skill.md"),
            r#"---
name: test-skill
description: A test skill
argument_hint: "name: string"
---

# Test Skill

Hello, {{name}}!

Context: {{context}}
"#,
        )
        .unwrap();

        let loader = SkillLoader::new(skills_dir);
        let executor = SkillExecutor::new(loader);

        (executor, temp_dir)
    }

    #[test]
    fn test_execute_with_args() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        fs::write(
            skills_dir.join("greet.md"),
            r#"---
name: greet
description: Greeting skill
---

Hello, {{name}}!
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        let mut executor = SkillExecutor::new(loader);

        let mut args = HashMap::new();
        args.insert("name".to_string(), "World".to_string());

        let result = executor.execute("greet", args, None).unwrap();

        assert_eq!(result.skill_name, "greet");
        assert!(result.output.contains("Hello, World!"));
        assert!(result.variables_set.contains(&"name".to_string()));
    }

    #[test]
    fn test_execute_with_context() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        fs::write(
            skills_dir.join("context-skill.md"),
            r#"---
name: context-skill
description: Context test
---

{{context}}
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        let mut executor = SkillExecutor::new(loader);

        let result = executor
            .execute(
                "context-skill",
                HashMap::new(),
                Some("Context value".to_string()),
            )
            .unwrap();

        assert!(result.output.contains("Context value"));
    }

    #[test]
    fn test_execute_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let loader = SkillLoader::new(skills_dir);
        let mut executor = SkillExecutor::new(loader);

        let result = executor.execute("nonexistent", HashMap::new(), None);
        assert!(matches!(result, Err(ExecutorError::SkillNotFound(_))));
    }

    #[test]
    fn test_format_template() {
        let temp_dir = TempDir::new().unwrap();
        let loader = SkillLoader::new(temp_dir.path());
        let mut executor = SkillExecutor::new(loader);

        executor.set_variable("name", "Alice".to_string());
        executor.set_variable("age", "30".to_string());

        let template = "Name: {{name}}, Age: {{age}}";
        let result = executor.format_template(template).unwrap();

        assert_eq!(result, "Name: Alice, Age: 30");
    }

    #[test]
    fn test_format_template_missing_var() {
        let temp_dir = TempDir::new().unwrap();
        let loader = SkillLoader::new(temp_dir.path());
        let executor = SkillExecutor::new(loader);

        // Missing variables should be replaced with empty string
        let template = "Hello, {{name}}!";
        let result = executor.format_template(template).unwrap();

        assert_eq!(result, "Hello, !");
    }

    #[test]
    fn test_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        fs::write(
            skills_dir.join("state-test.md"),
            r#"---
name: state-test
description: State test
---

{{var1}} {{var2}}
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        let mut executor = SkillExecutor::new(loader);

        // First execution
        let mut args1 = HashMap::new();
        args1.insert("var1".to_string(), "first".to_string());
        let result1 = executor.execute("state-test", args1, None).unwrap();
        assert!(result1.output.contains("first"));

        // Second execution with different args
        let mut args2 = HashMap::new();
        args2.insert("var2".to_string(), "second".to_string());
        let result2 = executor.execute("state-test", args2, None).unwrap();
        assert!(result2.output.contains("second"));
    }

    #[test]
    fn test_get_set_variables() {
        let temp_dir = TempDir::new().unwrap();
        let loader = SkillLoader::new(temp_dir.path());
        let mut executor = SkillExecutor::new(loader);

        executor.set_variable("foo", "bar".to_string());
        executor.set_variable("baz", "qux".to_string());

        assert_eq!(executor.get_variable("foo"), Some("bar".to_string()));
        assert_eq!(executor.get_variable("baz"), Some("qux".to_string()));
        assert_eq!(executor.get_variable("nonexistent"), None);
    }

    #[test]
    fn test_clear_state() {
        let temp_dir = TempDir::new().unwrap();
        let loader = SkillLoader::new(temp_dir.path());
        let mut executor = SkillExecutor::new(loader);

        executor.set_variable("test", "value".to_string());
        assert!(executor.get_variable("test").is_some());

        executor.clear_state();
        assert!(executor.get_variable("test").is_none());
    }
}
