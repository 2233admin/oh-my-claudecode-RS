use super::*;

pub(super) fn run_control_tool(command: &ToolCommand) -> Result<Option<Value>, DispatchError> {
    let response = match command {
        ToolCommand::Capabilities { request_id } => serde_json::to_value(ToolResponse::success(
            normalize_request_id(request_id.as_deref(), "cli-capabilities"),
            capabilities_payload(),
        ))?,
        ToolCommand::Route {
            task,
            agent_type,
            previous_failures,
            request_id,
        } => serde_json::to_value(ToolResponse::success(
            normalize_request_id(request_id.as_deref(), "cli-route"),
            route_agent_task(&RouteRequest {
                task: task.clone(),
                agent_type: agent_type.clone(),
                previous_failures: Some(*previous_failures),
            }),
        ))?,
        ToolCommand::CodeIntelQuery {
            repo,
            artifact_root,
            repo_path,
            artifact_schema,
            artifact_type,
            contains,
            artifact_uri,
            limit,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-code-intel");
            match query_code_intel(&CodeIntelQueryRequest {
                repo: repo.clone(),
                artifact_root: artifact_root.clone(),
                repo_path: repo_path.clone(),
                artifact_schema: artifact_schema.clone(),
                artifact_type: artifact_type.clone(),
                contains: contains.clone(),
                artifact_uri: artifact_uri.clone(),
                limit: *limit,
                request_id: Some(request_id.clone()),
            }) {
                Ok(payload) => serde_json::to_value(ToolResponse::success(request_id, payload))?,
                Err(error) => {
                    serde_json::to_value(ToolResponse::<Value>::failure(request_id, error))?
                }
            }
        }
        ToolCommand::TeamObservability {
            view,
            root,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-team-observability");
            match team_observability(Path::new(root), view) {
                Ok(payload) => serde_json::to_value(ToolResponse::success(request_id, payload))?,
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new("upstream_failed", error),
                ))?,
            }
        }
        ToolCommand::InteropSnapshot {
            root,
            limit,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-interop-snapshot");
            match read_snapshot(root, Some(*limit)) {
                Ok(payload) => serde_json::to_value(ToolResponse::success(request_id, payload))?,
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new("invalid_request", error.to_string()),
                ))?,
            }
        }
        ToolCommand::InteropBridge {
            action,
            source,
            target,
            task_type,
            description,
            content,
            root,
            allow_side_effects,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-interop-bridge");
            match interop_bridge_request_from_cli(InteropBridgeCliArgs {
                action,
                source,
                target,
                task_type: task_type.as_deref(),
                description: description.as_deref(),
                content: content.as_deref(),
                working_directory: root,
                allow_side_effects: *allow_side_effects,
            }) {
                Ok(request) => match interop_bridge(&request) {
                    Ok(payload) => {
                        serde_json::to_value(ToolResponse::success(request_id, payload))?
                    }
                    Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new(error.code(), error.to_string()),
                    ))?,
                },
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    ToolError::new(error.code(), error.to_string()),
                ))?,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}
