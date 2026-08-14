use clap::Parser;
use omc_cli::commands::{Cli, Commands};
use omc_cli::dispatch::run_tool_value;
use omc_mcp::McpTool;
use omc_mcp::agent_tools::WorkflowAdvanceTool;
use omc_mcp::python_tools::python_tools;
use serde_json::Value;
#[cfg(windows)]
use std::io::Write;
#[cfg(windows)]
use std::process::{Command, Stdio};

fn cli_response() -> Value {
    let cli = Cli::try_parse_from([
        "omc",
        "tool",
        "workflow-advance",
        "--current-stage",
        "planning",
        "--all-tasks-assigned",
        "--plan-approved",
        "--request-id",
        "cross-transport",
    ])
    .expect("CLI fixture parses");

    let Commands::Tool { command } = cli.command else {
        panic!("fixture must select a tool command");
    };
    run_tool_value(&command).expect("CLI tool response serializes")
}

fn mcp_response() -> Value {
    let result = WorkflowAdvanceTool.handle(serde_json::json!({
        "currentStage": "planning",
        "allTasksAssigned": true,
        "planApproved": true,
        "requestId": "cross-transport"
    }));
    assert_eq!(result.is_error, None);
    serde_json::from_str(&result.content[0].text).expect("MCP tool response is JSON")
}

#[test]
fn cli_and_mcp_emit_equivalent_workflow_envelopes() {
    assert_eq!(cli_response(), mcp_response());
}

#[test]
fn cli_team_observability_projects_existing_state_contract() {
    let root = tempfile::tempdir().expect("temporary project root");
    let cli = Cli::try_parse_from(vec![
        "omc".to_string(),
        "tool".to_string(),
        "team-observability".to_string(),
        "--view".to_string(),
        "sessions".to_string(),
        "--root".to_string(),
        root.path().to_string_lossy().into_owned(),
        "--request-id".to_string(),
        "cli-team-observability".to_string(),
    ])
    .expect("team observability CLI fixture parses");
    let Commands::Tool { command } = cli.command else {
        panic!("fixture must select a tool command");
    };
    let response = run_tool_value(&command).expect("team observability response serializes");
    assert_eq!(response["ok"], true);
    assert_eq!(
        response["data"]["schemaVersion"],
        "omc.team-observability.v1"
    );
    assert_eq!(response["data"]["view"], "sessions");
    assert_eq!(response["data"]["data"], serde_json::json!([]));
}

#[test]
fn cli_python_repl_requires_explicit_side_effect_opt_in() {
    let cli = Cli::try_parse_from([
        "omc",
        "tool",
        "python-repl",
        "--action",
        "execute",
        "--session-id",
        "contract-test",
        "--code",
        "print(1)",
    ])
    .expect("Python CLI fixture parses");
    let Commands::Tool { command } = cli.command else {
        panic!("fixture must select a tool command");
    };
    let response = run_tool_value(&command).expect("Python CLI response serializes");
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "side_effects_not_allowed");
}

#[test]
fn cli_debug_inspect_requires_explicit_side_effect_opt_in() {
    let cli = Cli::try_parse_from([
        "omc",
        "tool",
        "debug-inspect",
        "--adapter-command",
        "missing-dap-adapter",
        "--mode",
        "launch",
        "--action",
        "threads",
        "--launch-arguments",
        r#"{"program":"target"}"#,
    ])
    .expect("debug CLI fixture parses");
    let Commands::Tool { command } = cli.command else {
        panic!("fixture must select a tool command");
    };
    let response = run_tool_value(&command).expect("debug CLI response serializes");
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "side_effects_not_allowed");
}

#[test]
fn mcp_python_repl_keeps_session_state_when_available() {
    let tools = python_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition().name == "python_repl")
        .expect("python tool is registered");
    let first = tool.handle(serde_json::json!({
        "action": "execute",
        "sessionId": "contract-session",
        "code": "value = 41",
        "allowSideEffects": true,
        "requestId": "python-1"
    }));
    let first: Value = serde_json::from_str(&first.content[0].text).expect("first response JSON");
    if first["ok"] == false && first["error"]["code"] == "adapter_unavailable" {
        return;
    }
    assert_eq!(first["ok"], true);

    let second = tool.handle(serde_json::json!({
        "action": "execute",
        "sessionId": "contract-session",
        "code": "print(value + 1)",
        "allowSideEffects": true,
        "requestId": "python-2"
    }));
    let second: Value =
        serde_json::from_str(&second.content[0].text).expect("second response JSON");
    assert_eq!(second["ok"], true);
    assert_eq!(second["data"]["result"]["stdout"], "42\n");
}

#[cfg(windows)]
#[test]
fn unified_cli_starts_mcp_stdio_server() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omc"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("unified MCP server starts");
    let mut stdin = child.stdin.take().expect("MCP stdin is piped");
    let stdout = child.stdout.take().expect("MCP stdout is piped");
    let mut reader = std::io::BufReader::new(stdout);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
    )
    .expect("initialize request writes");
    stdin.flush().expect("initialize request flushes");
    let mut line = String::new();
    std::io::BufRead::read_line(&mut reader, &mut line).expect("initialize response reads");
    let initialize: Value = serde_json::from_str(&line).expect("initialize response is JSON");
    assert_eq!(initialize["result"]["serverInfo"]["name"], "omc-mcp");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
    )
    .expect("tools/list request writes");
    stdin.flush().expect("tools/list request flushes");
    line.clear();
    std::io::BufRead::read_line(&mut reader, &mut line).expect("tools/list response reads");
    let tools: Value = serde_json::from_str(&line).expect("tools/list response is JSON");
    let names = tools["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"agent_capabilities"));
    assert!(names.contains(&"python_repl"));
    assert!(names.contains(&"debug_inspect"));

    drop(stdin);
    assert!(child.wait().expect("MCP server exits").success());
}

#[cfg(windows)]
#[test]
fn codex_project_smoke_runs_clarify_plan_execute_verify() {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            PROJECT_SMOKE_SCRIPT,
        ])
        .env("OMC_SMOKE_COMMAND", env!("CARGO_BIN_EXE_omc"))
        .output()
        .expect("PowerShell smoke starts");
    assert!(
        output.status.success(),
        "Codex project smoke failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
const PROJECT_SMOKE_SCRIPT: &str = r###"
$ErrorActionPreference = 'Stop'
$OmCommand = $env:OMC_SMOKE_COMMAND
$project = Join-Path ([System.IO.Path]::GetTempPath()) ("omc-project-smoke-" + [guid]::NewGuid().ToString('N'))
$locationPushed = $false

function Invoke-Om {
    param([string[]]$Arguments)

    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $OmCommand
    $info.WorkingDirectory = (Get-Location).Path
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.Arguments = ($Arguments | ForEach-Object {
        $escaped = $_ -replace '(\\*)"', '$1$1\"'
        $escaped = $escaped -replace '(\\+)$', '$1$1'
        '"' + $escaped + '"'
    }) -join ' '
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $info
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    [pscustomobject]@{ ExitCode = $process.ExitCode; Stdout = $stdout; Stderr = $stderr }
}

function Invoke-OmJson {
    param([string[]]$Arguments)
    $result = Invoke-Om -Arguments $Arguments
    if ($result.ExitCode -ne 0) { throw "omc failed: $($result.Stderr)" }
    $result.Stdout | ConvertFrom-Json
}

function Assert-Equal {
    param($Actual, $Expected, [string]$Message)
    if ($Actual -ne $Expected) { throw "$Message; expected '$Expected', got '$Actual'" }
}

try {
    New-Item -ItemType Directory -Path (Join-Path $project 'src') -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $project 'src/main.rs') -Value 'fn main() { println!("before"); }' -NoNewline
    Push-Location $project
    $locationPushed = $true

    $setup = Invoke-Om -Arguments @('setup', '--host', 'codex', '--force')
    if ($setup.ExitCode -ne 0) { throw "Codex setup failed: $($setup.Stderr)" }
    foreach ($path in @('.codex/config.toml', '.codex/hooks.json', '.omc/skills')) {
        if (-not (Test-Path (Join-Path $project $path))) { throw "setup did not create $path" }
    }
    $codexConfig = Get-Content -LiteralPath (Join-Path $project '.codex/config.toml') -Raw
    if (-not $codexConfig.Contains('[mcp_servers.omc-rs]') -or
        -not $codexConfig.Contains('command = "omc"') -or
        -not $codexConfig.Contains('args = ["mcp"]')) {
        throw "setup did not register unified omc mcp server: $codexConfig"
    }

    $pythonDenied = Invoke-OmJson -Arguments @('tool', 'python-repl', '--action', 'execute', '--session-id', 'project-python', '--code', 'print(42)', '--request-id', 'project-python-denied')
    Assert-Equal $pythonDenied.error.code 'side_effects_not_allowed' 'python side-effect gate'
    $python = Invoke-OmJson -Arguments @('tool', 'python-repl', '--action', 'execute', '--session-id', 'project-python', '--code', 'print(42)', '--allow-side-effects', '--request-id', 'project-python')
    Assert-Equal $python.ok $true 'python execute'
    Assert-Equal $python.data.result.stdout.Trim() '42' 'python output'

    Invoke-OmJson -Arguments @('goal', 'create', '--id', 'project-smoke', '--objective', 'change and verify a small Rust project') | Out-Null
    Invoke-OmJson -Arguments @('goal', 'start', '--id', 'project-smoke') | Out-Null
    Invoke-OmJson -Arguments @('goal', 'checkpoint', '--id', 'project-smoke', '--checkpoint-id', 'clarified', '--summary', 'requirements clarified') | Out-Null
    $initializing = Invoke-OmJson -Arguments @('tool', 'workflow-advance', '--current-stage', 'initializing', '--requirements-clarified', '--request-id', 'project-clarify')
    Assert-Equal $initializing.data.nextStage 'planning' 'clarify stage'

    Invoke-OmJson -Arguments @('goal', 'checkpoint', '--id', 'project-smoke', '--checkpoint-id', 'planned', '--summary', 'plan approved and task assigned') | Out-Null
    $planning = Invoke-OmJson -Arguments @('tool', 'workflow-advance', '--current-stage', 'planning', '--all-tasks-assigned', '--plan-approved', '--request-id', 'project-plan')
    Assert-Equal $planning.data.nextStage 'executing' 'planning stage'

    $source = 'fn main() { println!("before"); }'
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $digest = ([BitConverter]::ToString($sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($source)))).Replace('-', '').ToLowerInvariant()
    $sha.Dispose()
    $anchors = ConvertTo-Json @(@{ line = 1; sha256 = $digest }) -Compress
    $edited = Invoke-OmJson -Arguments @(
        'tool', 'hash-edit', '--root', '.', '--path', 'src/main.rs', '--start-line', '1', '--end-line', '1',
        '--anchors-json', $anchors, '--replacement', 'fn main() { println!("after"); }', '--request-id', 'project-execute'
    )
    if ($edited.ok -ne $true -or $edited.data.applied -ne $true) { throw "hash edit failed: $($edited | ConvertTo-Json -Compress -Depth 8)" }
    Assert-Equal (Get-Content -LiteralPath (Join-Path $project 'src/main.rs') -Raw) 'fn main() { println!("after"); }' 'edited source'

    Invoke-OmJson -Arguments @('goal', 'checkpoint', '--id', 'project-smoke', '--checkpoint-id', 'executed', '--summary', 'hash edit applied') | Out-Null
    $executing = Invoke-OmJson -Arguments @('tool', 'workflow-advance', '--current-stage', 'executing', '--all-tasks-completed', '--request-id', 'project-execute-complete')
    Assert-Equal $executing.data.nextStage 'verifying' 'execution stage'

    $target = Join-Path $project 'target'
    New-Item -ItemType Directory -Path $target -Force | Out-Null
    $binary = Join-Path $target 'project-smoke.exe'
    & rustc (Join-Path $project 'src/main.rs') -o $binary 2>&1 | Out-String | Write-Output
    if ($LASTEXITCODE -ne 0) { throw 'rustc verification failed' }
    $runOutput = & $binary
    if ($LASTEXITCODE -ne 0) { throw 'project binary failed' }
    Assert-Equal ($runOutput -join '').Trim() 'after' 'project output'

    $verified = Invoke-OmJson -Arguments @(
        'tool', 'result-validate', '--result-type', 'omc.project.verification.v1',
        '--payload', '{"passed":true,"command":"rustc"}', '--required-fields', 'passed', '--request-id', 'project-verify'
    )
    Assert-Equal $verified.data.resultType 'omc.project.verification.v1' 'verification result'
    Invoke-OmJson -Arguments @('goal', 'checkpoint', '--id', 'project-smoke', '--checkpoint-id', 'verified', '--summary', 'rustc and project binary passed') | Out-Null

    $verifying = Invoke-OmJson -Arguments @('tool', 'workflow-advance', '--current-stage', 'verifying', '--verification-passed', '--request-id', 'project-verify-complete')
    Assert-Equal $verifying.data.nextStage 'completed' 'verification stage'
    $completed = Invoke-OmJson -Arguments @('goal', 'complete', '--id', 'project-smoke')
    Assert-Equal $completed.status 'completed' 'goal status'
    Assert-Equal $completed.checkpoints.Count 4 'checkpoint count'
}
finally {
    if ($locationPushed) { Pop-Location }
    if (Test-Path $project) { Remove-Item -LiteralPath $project -Recurse -Force }
}
"###;
