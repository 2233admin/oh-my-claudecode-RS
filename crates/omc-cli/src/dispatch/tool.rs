use super::*;

mod control;
use control::run_control_tool;

pub(super) fn run_tool(command: &ToolCommand) -> Result<(), DispatchError> {
    let response = run_tool_value(command)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

/// Execute a tool command without rendering it, for host and transport tests.
pub fn run_tool_value(command: &ToolCommand) -> Result<Value, DispatchError> {
    if let Some(response) = run_control_tool(command)? {
        return Ok(response);
    }
    let response = match command {
        ToolCommand::LspDocumentSymbols {
            root,
            file,
            timeout_ms,
            request_id,
        } => {
            let request_id =
                normalize_request_id(request_id.as_deref(), "cli-lsp-document-symbols");
            match query_document_symbols(&LspDocumentSymbolsRequest {
                working_directory: Some(root.clone()),
                file: file.clone(),
                timeout_ms: *timeout_ms,
            }) {
                Ok(payload) => serde_json::to_value(ToolResponse::success(request_id, payload))?,
                Err(error) => {
                    serde_json::to_value(ToolResponse::<Value>::failure(request_id, error))?
                }
            }
        }
        ToolCommand::DebugInspect {
            adapter_command,
            adapter_args_json,
            mode,
            action,
            root,
            launch_arguments,
            attach_arguments,
            thread_id,
            frame_id,
            variables_reference,
            timeout_ms,
            allow_side_effects,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-debug-inspect");
            let parse_json = |raw: &Option<String>, field: &str| -> Result<Option<Value>, Value> {
                raw.as_deref()
                    .map(|value| {
                        serde_json::from_str(value).map_err(|error| {
                            serde_json::to_value(ToolResponse::<Value>::failure(
                                request_id.clone(),
                                omc_shared::agent_tool::ToolError::new(
                                    "invalid_request",
                                    format!("{field} must be valid JSON: {error}"),
                                ),
                            ))
                            .unwrap_or_else(|_| json!({"ok": false}))
                        })
                    })
                    .transpose()
            };
            let adapter_args = match adapter_args_json {
                Some(raw) => match serde_json::from_str::<Vec<String>>(raw) {
                    Ok(args) => args,
                    Err(error) => {
                        return Ok(serde_json::to_value(ToolResponse::<Value>::failure(
                            request_id,
                            omc_shared::agent_tool::ToolError::new(
                                "invalid_request",
                                format!("adapterArgsJson must be a JSON array: {error}"),
                            ),
                        ))?);
                    }
                },
                None => Vec::new(),
            };
            let launch_arguments = match parse_json(launch_arguments, "launchArguments") {
                Ok(value) => value,
                Err(response) => return Ok(response),
            };
            let attach_arguments = match parse_json(attach_arguments, "attachArguments") {
                Ok(value) => value,
                Err(response) => return Ok(response),
            };
            let request = match serde_json::from_value::<DebugInspectRequest>(json!({
                "adapterCommand": adapter_command,
                "adapterArgs": adapter_args,
                "workingDirectory": root,
                "mode": mode,
                "action": action,
                "launchArguments": launch_arguments,
                "attachArguments": attach_arguments,
                "threadId": thread_id,
                "frameId": frame_id,
                "variablesReference": variables_reference,
                "timeoutMs": timeout_ms,
                "allowSideEffects": allow_side_effects
            })) {
                Ok(request) => request,
                Err(error) => {
                    return Ok(serde_json::to_value(ToolResponse::<Value>::failure(
                        request_id,
                        omc_shared::agent_tool::ToolError::new(
                            "invalid_request",
                            error.to_string(),
                        ),
                    ))?);
                }
            };
            match inspect_debug(&request) {
                Ok(payload) => serde_json::to_value(ToolResponse::success(request_id, payload))?,
                Err(error) => {
                    serde_json::to_value(ToolResponse::<Value>::failure(request_id, error))?
                }
            }
        }
        ToolCommand::PythonRepl {
            action,
            session_id,
            code,
            root,
            timeout_ms,
            allow_side_effects,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-python-repl");
            let action = match serde_json::from_value::<ReplAction>(Value::String(action.clone())) {
                Ok(action) => action,
                Err(error) => {
                    return Ok(serde_json::to_value(ToolResponse::<Value>::failure(
                        request_id,
                        omc_shared::agent_tool::ToolError::new(
                            "invalid_request",
                            error.to_string(),
                        ),
                    ))?);
                }
            };
            let request = PythonToolRequest {
                action: action.clone(),
                session_id: session_id.clone(),
                code: code.clone(),
                execution_timeout: *timeout_ms,
                project_dir: Some(root.clone()),
                allow_side_effects: *allow_side_effects,
            };
            let (input, allow_side_effects) = match request.into_input() {
                Ok(value) => value,
                Err(error) => {
                    return Ok(serde_json::to_value(ToolResponse::<Value>::failure(
                        request_id,
                        python_error(error),
                    ))?);
                }
            };
            if !matches!(action, ReplAction::GetState | ReplAction::ListSessions)
                && !allow_side_effects
            {
                return Ok(serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new(
                        omc_shared::agent_tool::error_codes::SIDE_EFFECTS_NOT_ALLOWED,
                        "allowSideEffects=true is required for Python execution or session mutation",
                    ),
                ))?);
            }
            let service = PythonReplService::new();
            let result = match action {
                ReplAction::Execute => service.execute(&input).and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|e| PythonSessionError::Failed(e.to_string()))
                }),
                ReplAction::GetState => service.state(session_id, Some(root)).and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|e| PythonSessionError::Failed(e.to_string()))
                }),
                ReplAction::Reset => service.reset(session_id, Some(root)).and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|e| PythonSessionError::Failed(e.to_string()))
                }),
                ReplAction::Interrupt => {
                    service.interrupt(session_id, Some(root)).and_then(|value| {
                        serde_json::to_value(value)
                            .map_err(|e| PythonSessionError::Failed(e.to_string()))
                    })
                }
                ReplAction::ListSessions => service.list_sessions().and_then(|value| {
                    serde_json::to_value(value)
                        .map_err(|e| PythonSessionError::Failed(e.to_string()))
                }),
                ReplAction::Close => service.close(session_id, Some(root)).and_then(|closed| {
                    serde_json::to_value(serde_json::json!({ "closed": closed }))
                        .map_err(|e| PythonSessionError::Failed(e.to_string()))
                }),
            };
            match result {
                Ok(result) => {
                    let project_dir = std::path::Path::new(
                        &input.project_dir.clone().unwrap_or_else(|| ".".into()),
                    )
                    .canonicalize()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                    serde_json::to_value(ToolResponse::success(
                        request_id,
                        PythonToolPayload {
                            operation: "python.eval",
                            action,
                            session_id: input.research_session_id,
                            project_dir,
                            session_scope: "cli-process",
                            result,
                            side_effects: if allow_side_effects {
                                vec!["local Python code may read/write files, spawn processes, or use network".into()]
                            } else {
                                Vec::new()
                            },
                        },
                    ))?
                }
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    python_error(error),
                ))?,
            }
        }
        ToolCommand::WorkflowAdvance {
            current_stage,
            requirements_clarified,
            all_tasks_assigned,
            plan_approved,
            all_tasks_completed,
            verification_passed,
            has_failures,
            has_blockers,
            fix_attempts,
            max_fix_attempts,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-workflow-advance");
            match serde_json::from_value::<WorkflowStage>(Value::String(current_stage.clone())) {
                Ok(stage) => serde_json::to_value(ToolResponse::success(
                    request_id,
                    advance_workflow(&WorkflowAdvanceRequest {
                        current_stage: stage,
                        context: WorkflowContext {
                            requirements_clarified: *requirements_clarified,
                            all_tasks_assigned: *all_tasks_assigned,
                            plan_approved: *plan_approved,
                            all_tasks_completed: *all_tasks_completed,
                            verification_passed: *verification_passed,
                            has_failures: *has_failures,
                            has_blockers: *has_blockers,
                            fix_attempts: *fix_attempts,
                            max_fix_attempts: *max_fix_attempts,
                        },
                    }),
                ))?,
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new("invalid_request", error.to_string()),
                ))?,
            }
        }
        ToolCommand::ResultValidate {
            result_type,
            payload,
            required_fields,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-result-validate");
            match serde_json::from_str::<Value>(payload) {
                Ok(payload) => {
                    let schema = ResultSchema::new(result_type.clone(), required_fields.clone());
                    match TypedSubagentResult::new(result_type.clone(), payload, Vec::new())
                        .and_then(|result| result.validate_against(&schema).map(|_| result))
                    {
                        Ok(result) => {
                            serde_json::to_value(ToolResponse::success(request_id, result))?
                        }
                        Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                            request_id,
                            omc_shared::agent_tool::ToolError::new("invalid_request", error),
                        ))?,
                    }
                }
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new("invalid_request", error.to_string()),
                ))?,
            }
        }
        ToolCommand::HashEdit {
            root,
            path,
            start_line,
            end_line,
            anchors_json,
            replacement,
            expected_file_sha256,
            request_id,
        } => {
            let request_id = normalize_request_id(request_id.as_deref(), "cli-hash-edit");
            match serde_json::from_str::<Vec<LineAnchor>>(anchors_json) {
                Ok(anchors) if !anchors.is_empty() => {
                    let mut edit =
                        HashEdit::new(path, *start_line, *end_line, anchors, replacement);
                    edit.expected_file_sha256 = expected_file_sha256.clone();
                    match edit.apply(Path::new(root)) {
                        Ok(result) => {
                            serde_json::to_value(ToolResponse::success(request_id, result))?
                        }
                        Err(error) => {
                            let code = if matches!(
                                error,
                                omc_shared::hash_edit::HashEditError::StaleAnchor { .. }
                                    | omc_shared::hash_edit::HashEditError::StaleFile { .. }
                            ) {
                                "stale_edit"
                            } else {
                                "edit_failed"
                            };
                            serde_json::to_value(ToolResponse::<Value>::failure(
                                request_id,
                                omc_shared::agent_tool::ToolError::new(code, error.to_string()),
                            ))?
                        }
                    }
                }
                Ok(_) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new(
                        "invalid_request",
                        "anchors must not be empty",
                    ),
                ))?,
                Err(error) => serde_json::to_value(ToolResponse::<Value>::failure(
                    request_id,
                    omc_shared::agent_tool::ToolError::new("invalid_request", error.to_string()),
                ))?,
            }
        }
        _ => {
            return Err(DispatchError::NotFound(
                "tool command was not handled by the CLI dispatcher".into(),
            ));
        }
    };

    Ok(response)
}

fn python_error(error: PythonSessionError) -> omc_shared::agent_tool::ToolError {
    let code = match &error {
        PythonSessionError::InvalidRequest(_) => {
            omc_shared::agent_tool::error_codes::INVALID_REQUEST
        }
        PythonSessionError::Unavailable(_) => {
            omc_shared::agent_tool::error_codes::ADAPTER_UNAVAILABLE
        }
        PythonSessionError::Timeout(_) => "execution_timeout",
        PythonSessionError::Failed(_) => omc_shared::agent_tool::error_codes::UPSTREAM_FAILED,
    };
    omc_shared::agent_tool::ToolError::new(code, error.to_string())
}
