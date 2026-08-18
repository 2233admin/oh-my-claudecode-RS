//! Durable project-goal MCP tools.
//!
//! These tools persist goal control-plane state only. They do not dispatch an
//! agent, call a provider, or emulate a task executor.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use omc_shared::operation_contract::ArtifactRef;
use omc_shared::{GoalCheckpoint, GoalLedger, GoalRecord, OmcPaths};
use serde::Serialize;
use serde_json::Value;

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

fn string_property(description: &str) -> SchemaProperty {
    SchemaProperty {
        prop_type: "string".into(),
        description: Some(description.into()),
        r#enum: None,
        max_length: None,
        minimum: None,
        maximum: None,
    }
}

fn array_property(description: &str) -> SchemaProperty {
    SchemaProperty {
        prop_type: "array".into(),
        description: Some(description.into()),
        r#enum: None,
        max_length: None,
        minimum: None,
        maximum: None,
    }
}

fn object_schema(properties: HashMap<String, SchemaProperty>, required: Vec<String>) -> ToolSchema {
    ToolSchema {
        schema_type: "object".into(),
        properties,
        required,
    }
}

fn working_directory(properties: &mut HashMap<String, SchemaProperty>) {
    properties.insert(
        "workingDirectory".into(),
        string_property("Project root. Defaults to the current directory."),
    );
}

fn ledger(args: &Value) -> GoalLedger {
    let cwd = args
        .get("workingDirectory")
        .and_then(Value::as_str)
        .unwrap_or(".");
    GoalLedger::new(OmcPaths::new_with_root(PathBuf::from(cwd).join(".omc")))
}

fn required_string<'a>(args: &'a Value, field: &str) -> Result<&'a str, ToolResult> {
    args.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolResult::error(format!("Missing required parameter: {field}")))
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn encode<T: Serialize>(value: &T) -> ToolResult {
    match serde_json::to_string_pretty(value) {
        Ok(text) => ToolResult::ok(text),
        Err(error) => ToolResult::error(format!("failed to encode goal response: {error}")),
    }
}

fn encode_error(error: impl std::fmt::Display) -> ToolResult {
    ToolResult::error(format!("goal operation failed: {error}"))
}

pub struct GoalCreateTool;

impl McpTool for GoalCreateTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("goalId".into(), string_property("Stable goal identifier."));
        properties.insert("objective".into(), string_property("Goal objective."));
        properties.insert(
            "owner".into(),
            string_property("Optional agency or human owner."),
        );
        properties.insert(
            "taskId".into(),
            string_property("Optional task ID used for dispatch correlation."),
        );
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_create".into(),
            description: "Create a planned project goal in the durable OMC ledger.".into(),
            input_schema: object_schema(properties, vec!["goalId".into(), "objective".into()]),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let goal_id = match required_string(&args, "goalId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let objective = match required_string(&args, "objective") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let mut goal = GoalRecord::new(goal_id, objective, now());
        goal.owner = args
            .get("owner")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        if let Some(task_id) = args.get("taskId").and_then(Value::as_str)
            && let Err(error) = goal.attach_task(task_id)
        {
            return encode_error(error);
        }
        match ledger(&args).create(&goal) {
            Ok(()) => encode(&goal),
            Err(error) => encode_error(error),
        }
    }
}

pub struct GoalListTool;

impl McpTool for GoalListTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_list".into(),
            description: "List durable project goals, sorted by goal ID.".into(),
            input_schema: object_schema(properties, Vec::new()),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        match ledger(&args).list() {
            Ok(goals) => encode(&goals),
            Err(error) => encode_error(error),
        }
    }
}

pub struct GoalGetTool;

impl McpTool for GoalGetTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("goalId".into(), string_property("Stable goal identifier."));
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_get".into(),
            description: "Read one durable project goal.".into(),
            input_schema: object_schema(properties, vec!["goalId".into()]),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let goal_id = match required_string(&args, "goalId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        match ledger(&args).load(goal_id) {
            Ok(goal) => encode(&goal),
            Err(error) => encode_error(error),
        }
    }
}

pub struct GoalStartTool;

impl McpTool for GoalStartTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("goalId".into(), string_property("Stable goal identifier."));
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_start".into(),
            description: "Resume a planned or blocked goal as active.".into(),
            input_schema: object_schema(properties, vec!["goalId".into()]),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let goal_id = match required_string(&args, "goalId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let ledger = ledger(&args);
        let mut goal = match ledger.load(goal_id) {
            Ok(goal) => goal,
            Err(error) => return encode_error(error),
        };
        if let Err(error) = goal.start(now()) {
            return encode_error(error);
        }
        match ledger.save(&goal) {
            Ok(()) => encode(&goal),
            Err(error) => encode_error(error),
        }
    }
}

pub struct GoalBlockTool;

impl McpTool for GoalBlockTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("goalId".into(), string_property("Stable goal identifier."));
        properties.insert("reason".into(), string_property("Durable blocker reason."));
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_block".into(),
            description: "Persist a blocker and mark a goal blocked.".into(),
            input_schema: object_schema(properties, vec!["goalId".into(), "reason".into()]),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let goal_id = match required_string(&args, "goalId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let reason = match required_string(&args, "reason") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let ledger = ledger(&args);
        let mut goal = match ledger.load(goal_id) {
            Ok(goal) => goal,
            Err(error) => return encode_error(error),
        };
        if let Err(error) = goal.block(reason, now()) {
            return encode_error(error);
        }
        match ledger.save(&goal) {
            Ok(()) => encode(&goal),
            Err(error) => encode_error(error),
        }
    }
}

pub struct GoalCheckpointTool;

impl McpTool for GoalCheckpointTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("goalId".into(), string_property("Stable goal identifier."));
        properties.insert(
            "checkpointId".into(),
            string_property("Unique checkpoint identifier within the goal."),
        );
        properties.insert("summary".into(), string_property("Checkpoint summary."));
        properties.insert(
            "artifactRefs".into(),
            array_property("Optional normalized artifact references."),
        );
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_checkpoint".into(),
            description: "Append a durable checkpoint, optionally linking normalized artifacts."
                .into(),
            input_schema: object_schema(
                properties,
                vec!["goalId".into(), "checkpointId".into(), "summary".into()],
            ),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let goal_id = match required_string(&args, "goalId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let checkpoint_id = match required_string(&args, "checkpointId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let summary = match required_string(&args, "summary") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let artifact_refs = match args.get("artifactRefs") {
            Some(value) => match serde_json::from_value::<Vec<ArtifactRef>>(value.clone()) {
                Ok(refs) => refs
                    .into_iter()
                    .map(ArtifactRef::with_canonical_uri)
                    .collect(),
                Err(error) => return encode_error(error),
            },
            None => Vec::new(),
        };
        let ledger = ledger(&args);
        let mut goal = match ledger.load(goal_id) {
            Ok(goal) => goal,
            Err(error) => return encode_error(error),
        };
        if let Err(error) = goal.add_checkpoint(GoalCheckpoint {
            checkpoint_id: checkpoint_id.to_string(),
            summary: summary.to_string(),
            recorded_at: now(),
            artifact_refs,
        }) {
            return encode_error(error);
        }
        match ledger.save(&goal) {
            Ok(()) => encode(&goal),
            Err(error) => encode_error(error),
        }
    }
}

pub struct GoalCompleteTool;

impl McpTool for GoalCompleteTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("goalId".into(), string_property("Stable goal identifier."));
        working_directory(&mut properties);
        ToolDefinition {
            name: "goal_complete".into(),
            description: "Mark an active goal completed.".into(),
            input_schema: object_schema(properties, vec!["goalId".into()]),
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let goal_id = match required_string(&args, "goalId") {
            Ok(value) => value,
            Err(error) => return error,
        };
        let ledger = ledger(&args);
        let mut goal = match ledger.load(goal_id) {
            Ok(goal) => goal,
            Err(error) => return encode_error(error),
        };
        if let Err(error) = goal.complete(now()) {
            return encode_error(error);
        }
        match ledger.save(&goal) {
            Ok(()) => encode(&goal),
            Err(error) => encode_error(error),
        }
    }
}

pub fn goal_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(GoalCreateTool),
        Box::new(GoalListTool),
        Box::new(GoalGetTool),
        Box::new(GoalStartTool),
        Box::new(GoalBlockTool),
        Box::new(GoalCheckpointTool),
        Box::new(GoalCompleteTool),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_tool_rejects_missing_objective() {
        let result = GoalCreateTool.handle(serde_json::json!({"goalId": "goal-1"}));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("objective"));
    }

    #[test]
    fn goal_tool_definitions_are_unique() {
        let tools = goal_tools();
        let mut names: Vec<_> = tools.iter().map(|tool| tool.definition().name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), tools.len());
    }
}
