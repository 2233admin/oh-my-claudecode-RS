use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{
    Mission, RunRecord, StartOptions, TaskCard, TrackerKind, new_run_id, slug, unix_timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    Claude,
    Fsc,
    Kohaku,
}

impl RuntimeKind {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Self::Claude),
            "fsc" | "full-self-coding" => Ok(Self::Fsc),
            "kohaku" | "kohakuterrarium" | "kt" => Ok(Self::Kohaku),
            _ => Err(format!("unknown runtime: {raw}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Fsc => "fsc",
            Self::Kohaku => "kohaku",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDoctorReport {
    pub runtime: RuntimeKind,
    pub ok: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRunRecord {
    pub record_type: String,
    pub run_id: String,
    pub runtime: RuntimeKind,
    pub team_name: String,
    pub task_id: String,
    pub mission_path: String,
    pub artifact_path: String,
    pub started_at: u64,
    pub launch_command: Vec<String>,
    pub tracker: Option<TrackerKind>,
    pub tracker_run_id: Option<String>,
    pub tracker_team_name: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStartReport {
    pub runtime: RuntimeKind,
    pub run_id: String,
    pub team_name: String,
    pub artifact_path: PathBuf,
    pub launch_command: Vec<String>,
}

#[derive(Debug, Clone)]
struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

trait RuntimeCommandRunner {
    fn output(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, String>;
}

struct SystemCommandRunner;

impl RuntimeCommandRunner for SystemCommandRunner {
    fn output(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, String> {
        let mut command = Command::new(program);
        command.args(args);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let output = command
            .output()
            .map_err(|_| format!("{program} not found on PATH"))?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

pub fn runtime_doctor(root: &Path, runtime: RuntimeKind) -> RuntimeDoctorReport {
    runtime_doctor_with_runner(root, runtime, &SystemCommandRunner)
}

pub fn check_runtime_ready(root: &Path, runtime: RuntimeKind) -> Result<(), String> {
    match runtime {
        RuntimeKind::Claude => crate::check_claude_ready(),
        RuntimeKind::Fsc => check_fsc_ready_with_runner(root, &SystemCommandRunner).map(|_| ()),
        RuntimeKind::Kohaku => {
            check_kohaku_ready_with_runner(root, &SystemCommandRunner).map(|_| ())
        }
    }
}

pub fn start_runtime(
    root: &Path,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    tracker_record: Option<&RunRecord>,
    mission_path: &Path,
) -> Result<RuntimeStartReport, String> {
    start_runtime_with_runner(
        root,
        mission,
        task,
        opts,
        tracker_record,
        mission_path,
        &SystemCommandRunner,
    )
}

pub fn collect_runtime_handoff(
    root: &Path,
    target: &str,
    runtime: RuntimeKind,
) -> Result<String, String> {
    let record = find_runtime_record(root, target, Some(runtime))?;
    let artifact_root = PathBuf::from(&record.artifact_path);
    let mut sections = Vec::new();

    if artifact_root.exists() {
        collect_artifact_sections(&artifact_root, &mut sections)?;
    }

    let generated_at = unix_timestamp().to_string();
    let summary = if sections.is_empty() {
        format!(
            "# OMC Runtime Handoff: {}\n\nGenerated: {generated_at}\nRuntime: {}\nRun: {}\n\nNo runtime artifacts were found. Ask the runtime lead to add a handoff or session export under `{}`.",
            record.team_name,
            record.runtime.as_str(),
            record.run_id,
            record.artifact_path
        )
    } else {
        format!(
            "# OMC Runtime Handoff: {}\n\nGenerated: {generated_at}\nRuntime: {}\nRun: {}\n\n{}",
            record.team_name,
            record.runtime.as_str(),
            record.run_id,
            sections.join("\n\n")
        )
    };

    let dir = root.join(".omc/team/handoffs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{}.md", record.team_name)), &summary).map_err(|e| e.to_string())?;
    Ok(summary)
}

pub fn find_runtime_record(
    root: &Path,
    target: &str,
    runtime: Option<RuntimeKind>,
) -> Result<RuntimeRunRecord, String> {
    let dir = root.join(".omc/team/runs");
    let mut matches = Vec::new();
    if !dir.exists() {
        return Err(format!("no runtime run records found for {target}"));
    }
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let Ok(record) = serde_json::from_str::<RuntimeRunRecord>(&raw) else {
            continue;
        };
        if record.record_type != "runtime" {
            continue;
        }
        if runtime.is_some_and(|kind| kind != record.runtime) {
            continue;
        }
        if record.run_id == target || record.team_name == target {
            matches.push(record);
        }
    }
    matches.sort_by_key(|record| record.started_at);
    matches
        .pop()
        .ok_or_else(|| format!("no runtime run record found for {target}"))
}

fn runtime_doctor_with_runner(
    root: &Path,
    runtime: RuntimeKind,
    runner: &dyn RuntimeCommandRunner,
) -> RuntimeDoctorReport {
    match runtime {
        RuntimeKind::Claude => {
            let mut messages = Vec::new();
            let env_ok = env::var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS")
                .ok()
                .as_deref()
                == Some("1");
            if env_ok {
                messages.push("env ok: CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1".to_string());
            } else {
                messages
                    .push("missing env: set CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1".to_string());
            }
            match runner.output("claude", &["--version"], None) {
                Ok(output) if output.success => {
                    let version = first_non_empty(&output.stdout, &output.stderr);
                    messages.push(format!("claude ok: {}", version));
                    if !crate::version_at_least(version, (2, 1, 32)) {
                        messages.push(format!(
                            "claude version too old for agent teams: {} (need v2.1.32+)",
                            version
                        ));
                    }
                }
                Ok(output) => {
                    messages.push(format!(
                        "claude --version failed: {}",
                        first_non_empty(&output.stderr, &output.stdout)
                    ));
                }
                Err(err) => messages.push(format!(
                    "{err}. Install Claude Code v2.1.32+ and ensure `claude` is on PATH"
                )),
            }
            RuntimeDoctorReport {
                runtime,
                ok: messages.iter().all(|message| {
                    !message.starts_with("missing")
                        && !message.contains("failed")
                        && !message.contains("not found")
                        && !message.contains("too old")
                }),
                messages,
            }
        }
        RuntimeKind::Fsc => match check_fsc_ready_with_runner(root, runner) {
            Ok(messages) => RuntimeDoctorReport {
                runtime,
                ok: true,
                messages,
            },
            Err(message) => RuntimeDoctorReport {
                runtime,
                ok: false,
                messages: vec![message],
            },
        },
        RuntimeKind::Kohaku => match check_kohaku_ready_with_runner(root, runner) {
            Ok(messages) => RuntimeDoctorReport {
                runtime,
                ok: true,
                messages,
            },
            Err(message) => RuntimeDoctorReport {
                runtime,
                ok: false,
                messages: vec![message],
            },
        },
    }
}

fn start_runtime_with_runner(
    root: &Path,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    tracker_record: Option<&RunRecord>,
    mission_path: &Path,
    runner: &dyn RuntimeCommandRunner,
) -> Result<RuntimeStartReport, String> {
    if opts.runtime == RuntimeKind::Claude {
        return Err("claude runtime is handled by Claude Code directly".to_string());
    }
    match opts.runtime {
        RuntimeKind::Claude => unreachable!(),
        RuntimeKind::Fsc => {
            check_fsc_ready_with_runner(root, runner)?;
            render_fsc_runtime(root, mission, task, opts, tracker_record, mission_path)
        }
        RuntimeKind::Kohaku => {
            check_kohaku_ready_with_runner(root, runner)?;
            render_kohaku_runtime(root, mission, task, opts, tracker_record, mission_path)
        }
    }
}

fn render_fsc_runtime(
    root: &Path,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    tracker_record: Option<&RunRecord>,
    mission_path: &Path,
) -> Result<RuntimeStartReport, String> {
    let fsc_root = detect_fsc_root(root)?;
    let record = base_runtime_record(root, mission, task, opts, tracker_record, mission_path)?;
    let artifact_root = PathBuf::from(&record.artifact_path);
    fs::create_dir_all(&artifact_root).map_err(|e| e.to_string())?;
    fs::write(
        artifact_root.join("mission.md"),
        runtime_mission_body(task, opts, "FSC swarm execution backend"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        artifact_root.join("task.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "run_id": record.run_id,
            "runtime": "fsc",
            "task_id": task.meta.id,
            "title": task.meta.title,
            "risk": task.meta.risk,
            "ownership": task.meta.ownership,
            "acceptance": task.meta.acceptance,
            "verification": task.meta.verification,
            "mission_path": mission_path.display().to_string(),
            "handoff_path": artifact_root.join("handoff.md").display().to_string()
        }))
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        artifact_root.join("handoff.md"),
        "# FSC Handoff\n\nFill this after FSC finishes: summary, changed files, validation, risks, and follow-ups.\n",
    )
    .map_err(|e| e.to_string())?;

    let launch_command = vec![
        "bun".to_string(),
        "--cwd".to_string(),
        fsc_root.display().to_string(),
        "run".to_string(),
        "start".to_string(),
        "--task".to_string(),
        artifact_root.join("task.json").display().to_string(),
    ];
    let mut record = record;
    record.launch_command = launch_command.clone();
    save_runtime_record(root, &record)?;
    Ok(RuntimeStartReport {
        runtime: RuntimeKind::Fsc,
        run_id: record.run_id,
        team_name: record.team_name,
        artifact_path: artifact_root,
        launch_command,
    })
}

fn render_kohaku_runtime(
    root: &Path,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    tracker_record: Option<&RunRecord>,
    mission_path: &Path,
) -> Result<RuntimeStartReport, String> {
    let record = base_runtime_record(root, mission, task, opts, tracker_record, mission_path)?;
    let artifact_root = PathBuf::from(&record.artifact_path);
    let terrarium_root = artifact_root.join("terrariums/omc_team");
    fs::create_dir_all(terrarium_root.join("prompts")).map_err(|e| e.to_string())?;
    fs::write(
        artifact_root.join("kohaku.yaml"),
        render_kohaku_manifest(&record),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        terrarium_root.join("terrarium.yaml"),
        render_kohaku_terrarium(opts.team_size),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        terrarium_root.join("prompts/root.md"),
        render_kohaku_root_prompt(task, mission_path),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        artifact_root.join("seed.md"),
        runtime_mission_body(task, opts, "KohakuTerrarium creature team"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        artifact_root.join("handoff.md"),
        "# Kohaku Handoff\n\nFill this after the terrarium finishes: summary, changed files, validation, risks, and follow-ups.\n",
    )
    .map_err(|e| e.to_string())?;

    for creature in kohaku_creatures(opts.team_size) {
        let creature_root = terrarium_root.join(format!("creatures/{}", creature.name));
        fs::create_dir_all(creature_root.join("prompts")).map_err(|e| e.to_string())?;
        fs::write(
            creature_root.join("config.yaml"),
            render_kohaku_creature_config(&creature),
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            creature_root.join("prompts/system.md"),
            render_kohaku_creature_prompt(&creature, task, mission_path),
        )
        .map_err(|e| e.to_string())?;
    }

    let launch_command = vec![
        "kt".to_string(),
        "terrarium".to_string(),
        "run".to_string(),
        terrarium_root.display().to_string(),
        "--mode".to_string(),
        "tui".to_string(),
        "--seed".to_string(),
        format!(
            "Run OMC mission {}. Full seed: {}",
            task.meta.id,
            artifact_root.join("seed.md").display()
        ),
    ];
    let mut record = record;
    record.launch_command = launch_command.clone();
    save_runtime_record(root, &record)?;
    Ok(RuntimeStartReport {
        runtime: RuntimeKind::Kohaku,
        run_id: record.run_id,
        team_name: record.team_name,
        artifact_path: artifact_root,
        launch_command,
    })
}

fn base_runtime_record(
    root: &Path,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    tracker_record: Option<&RunRecord>,
    mission_path: &Path,
) -> Result<RuntimeRunRecord, String> {
    let run_id = tracker_record.map_or_else(
        || new_run_id(&format!("{}-{}", mission.id, opts.runtime.as_str())),
        |record| format!("{}-{}", record.run_id, opts.runtime.as_str()),
    );
    let artifact_path = root
        .join(format!(".omc/team/{}/{}", opts.runtime.as_str(), run_id))
        .display()
        .to_string();
    Ok(RuntimeRunRecord {
        record_type: "runtime".to_string(),
        run_id,
        runtime: opts.runtime,
        team_name: mission.team_name.clone(),
        task_id: task.meta.id.clone(),
        mission_path: mission_path.display().to_string(),
        artifact_path,
        started_at: unix_timestamp(),
        launch_command: Vec::new(),
        tracker: tracker_record.map(|record| record.tracker),
        tracker_run_id: tracker_record.map(|record| record.run_id.clone()),
        tracker_team_name: tracker_record.map(|record| record.team_name.clone()),
        status: "prepared".to_string(),
    })
}

fn save_runtime_record(root: &Path, record: &RuntimeRunRecord) -> Result<(), String> {
    let dir = root.join(".omc/team/runs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", record.run_id));
    let rendered = serde_json::to_string_pretty(record).map_err(|e| e.to_string())? + "\n";
    fs::write(path, rendered).map_err(|e| e.to_string())
}

fn check_kohaku_ready_with_runner(
    root: &Path,
    runner: &dyn RuntimeCommandRunner,
) -> Result<Vec<String>, String> {
    let version = runner
        .output("kt", &["--version"], None)
        .map_err(|err| format!("{err}. Install KohakuTerrarium and run `kt --version`."))?;
    if !version.success {
        return Err(format!(
            "kt --version failed: {}",
            first_non_empty(&version.stderr, &version.stdout)
        ));
    }
    let mut messages = vec![format!(
        "kt ok: {}",
        first_non_empty(&version.stdout, &version.stderr)
    )];
    match runner.output("kt", &["list"], None) {
        Ok(output) if output.success && output.stdout.contains("kt-biome") => {
            messages.push("kt-biome package ok".to_string());
        }
        Ok(output) if output.success => {
            messages.push(
                "kt-biome package not detected; install with `kt install https://github.com/Kohaku-Lab/kt-biome.git` if base creatures are missing"
                    .to_string(),
            );
        }
        Ok(output) => messages.push(format!(
            "kt list warning: {}",
            first_non_empty(&output.stderr, &output.stdout)
        )),
        Err(err) => messages.push(format!("kt list warning: {err}")),
    }
    messages.push(format!(
        "artifact root: {}",
        root.join(".omc/team/kohaku").display()
    ));
    Ok(messages)
}

fn check_fsc_ready_with_runner(
    root: &Path,
    runner: &dyn RuntimeCommandRunner,
) -> Result<Vec<String>, String> {
    let fsc_root = detect_fsc_root(root)?;
    let package_json = fsc_root.join("package.json");
    if !package_json.exists() {
        return Err(format!(
            "FSC root {} is missing package.json",
            fsc_root.display()
        ));
    }
    let bun = runner.output("bun", &["--version"], None).map_err(|err| {
        format!("{err}. Install Bun or expose it on PATH before using the FSC runtime.")
    })?;
    if !bun.success {
        return Err(format!(
            "bun --version failed: {}",
            first_non_empty(&bun.stderr, &bun.stdout)
        ));
    }
    let mut messages = vec![
        format!("FSC root ok: {}", fsc_root.display()),
        format!("bun ok: {}", first_non_empty(&bun.stdout, &bun.stderr)),
    ];
    match runner.output("docker", &["--version"], None) {
        Ok(output) if output.success => messages.push(format!(
            "docker ok: {}",
            first_non_empty(&output.stdout, &output.stderr)
        )),
        _ => messages.push(
            "docker not detected; FSC bare mode may still work if configured locally".to_string(),
        ),
    }
    Ok(messages)
}

fn detect_fsc_root(root: &Path) -> Result<PathBuf, String> {
    for key in ["OMC_FSC_ROOT", "FSC_ROOT", "FULL_SELF_CODING_ROOT"] {
        if let Ok(value) = env::var(key)
            && !value.trim().is_empty()
        {
            let path = PathBuf::from(value);
            if path.exists() {
                return Ok(path);
            }
            return Err(format!("{key} points to missing path: {}", path.display()));
        }
    }

    let candidates = [
        root.join("full-self-coding"),
        root.parent().map_or_else(
            || root.join("../full-self-coding"),
            |parent| parent.join("full-self-coding"),
        ),
        root.parent()
            .map_or_else(|| root.join("../FSC"), |parent| parent.join("FSC")),
    ];
    candidates
        .into_iter()
        .find(|path| path.join("package.json").exists())
        .ok_or_else(|| {
            "FSC root not found. Set OMC_FSC_ROOT, FSC_ROOT, or FULL_SELF_CODING_ROOT to a local full-self-coding checkout."
                .to_string()
        })
}

fn render_kohaku_manifest(record: &RuntimeRunRecord) -> String {
    format!(
        r#"name: omc-team-{package}
version: "0.1.0"
description: "Generated OMC runtime package for run {run_id}"
terrariums:
  - name: omc_team
"#,
        package = slug(&record.run_id),
        run_id = record.run_id
    )
}

fn render_kohaku_terrarium(team_size: u8) -> String {
    let mut creatures = vec![
        r#"    - name: developer
      config: ./creatures/developer/
      output_wiring: [reviewer]
      channels:
        listen: [tasks, feedback, team_chat]
        can_send: [review, status, team_chat]
"#
        .to_string(),
        r#"    - name: reviewer
      config: ./creatures/reviewer/
      channels:
        listen: [review, test_results, security_findings, team_chat]
        can_send: [feedback, approved, status, team_chat]
"#
        .to_string(),
        r#"    - name: tester
      config: ./creatures/tester/
      channels:
        listen: [approved, team_chat]
        can_send: [test_results, results, status, team_chat]
"#
        .to_string(),
    ];
    if team_size >= 4 {
        creatures.push(
            r#"    - name: security
      config: ./creatures/security/
      channels:
        listen: [review, approved, team_chat]
        can_send: [security_findings, feedback, status, team_chat]
"#
            .to_string(),
        );
    }
    if team_size >= 5 {
        creatures.push(
            r#"    - name: integrator
      config: ./creatures/integrator/
      channels:
        listen: [results, status, team_chat]
        can_send: [results, status, team_chat]
"#
            .to_string(),
        );
    }
    format!(
        r#"terrarium:
  name: omc_team

  root:
    base_config: "@kt-biome/creatures/general"
    system_prompt_file: prompts/root.md

  creatures:
{creatures}
  channels:
    tasks:        {{ type: queue, description: "OMC mission tasks" }}
    review:       {{ type: queue, description: "Implementation output for review" }}
    feedback:     {{ type: queue, description: "Review or audit feedback" }}
    approved:     {{ type: queue, description: "Approved work for validation" }}
    test_results: {{ type: queue, description: "Validation results" }}
    security_findings: {{ type: queue, description: "Security audit findings" }}
    results:      {{ type: queue, description: "Final verified handoff" }}
    status:       {{ type: broadcast, description: "Team progress updates" }}
    team_chat:    {{ type: broadcast, description: "Peer coordination" }}
"#,
        creatures = creatures.join("")
    )
}

#[derive(Debug, Clone, Copy)]
struct KohakuCreature {
    name: &'static str,
    base_config: &'static str,
    role: &'static str,
}

fn kohaku_creatures(team_size: u8) -> Vec<KohakuCreature> {
    let mut out = vec![
        KohakuCreature {
            name: "developer",
            base_config: "@kt-biome/creatures/swe",
            role: "Implement the accepted task slice and keep changes surgical.",
        },
        KohakuCreature {
            name: "reviewer",
            base_config: "@kt-biome/creatures/critic",
            role: "Review implementation output for correctness, scope, and verification.",
        },
        KohakuCreature {
            name: "tester",
            base_config: "@kt-biome/creatures/swe",
            role: "Run or specify validation and return test results plus residual risk.",
        },
    ];
    if team_size >= 4 {
        out.push(KohakuCreature {
            name: "security",
            base_config: "@kt-biome/creatures/critic",
            role: "Audit risky changes, secrets, command execution, auth, and destructive behavior.",
        });
    }
    if team_size >= 5 {
        out.push(KohakuCreature {
            name: "integrator",
            base_config: "@kt-biome/creatures/swe",
            role: "Synthesize verified results into one OMC handoff without doing unrelated work.",
        });
    }
    out
}

fn render_kohaku_creature_config(creature: &KohakuCreature) -> String {
    format!(
        r#"name: {name}
base_config: "{base_config}"
prompt_mode: replace
system_prompt_file: prompts/system.md
"#,
        name = creature.name,
        base_config = creature.base_config
    )
}

fn render_kohaku_root_prompt(task: &TaskCard, mission_path: &Path) -> String {
    format!(
        r#"You are the OMC Lead running a KohakuTerrarium team.

Mission: {id} — {title}
Mission file: {mission_path}

Architecture boundaries:
- You are the root creature outside the terrarium, not a peer creature.
- The terrarium is wiring, lifecycle, and channels only; do not treat it as an LLM decision maker.
- Peer creatures are horizontally composed and opaque to each other.
- Private sub-agents are vertical delegation inside one creature; do not mix them with peer creature coordination.
- Keep prompts focused on role and project rules. Do not inline tool lists, tool call syntax, or full tool documentation.

Operating contract:
- Seed work through the tasks queue.
- Use status broadcasts for progress visibility.
- Require reviewer approval before validation.
- Require final results to include summary, changed files, validation, risks, and follow-ups.
- Do not modify KohakuTerrarium upstream or open an upstream PR from this run.
"#,
        id = task.meta.id,
        title = task.meta.title,
        mission_path = mission_path.display()
    )
}

fn render_kohaku_creature_prompt(
    creature: &KohakuCreature,
    task: &TaskCard,
    mission_path: &Path,
) -> String {
    format!(
        r#"You are the OMC Kohaku creature `{name}`.

Role: {role}
Mission: {id} — {title}
Mission file: {mission_path}

Kohaku boundaries:
- You are a self-contained creature with your own prompt, memory, I/O, and inherited runtime modules.
- You do not know or manage the terrarium internals; communicate only through configured channels.
- Do not treat peer creatures as private sub-agents.
- Do not manually document tools or call syntax in your output.

OMC discipline:
- Think before coding, keep changes simple, and touch only task-relevant files.
- Preserve user changes and unrelated code.
- Report verification evidence before marking work complete.
"#,
        name = creature.name,
        role = creature.role,
        id = task.meta.id,
        title = task.meta.title,
        mission_path = mission_path.display()
    )
}

fn runtime_mission_body(task: &TaskCard, opts: &StartOptions, runtime_name: &str) -> String {
    format!(
        r#"# OMC Runtime Mission

Runtime: {runtime_name}
Team size: {team_size}

## Task

- ID: {id}
- Title: {title}
- Risk: {risk}

## Ownership

{ownership}

## Acceptance

{acceptance}

## Verification

{verification}

## Context

{body}

## Handoff Contract

- Summarize what changed.
- List changed files or generated artifacts.
- Include validation commands and results.
- Call out residual risks, blockers, and follow-ups.
- Do not create upstream FSC or Kohaku PRs from this run.
"#,
        runtime_name = runtime_name,
        team_size = opts.team_size,
        id = task.meta.id,
        title = task.meta.title,
        risk = task.meta.risk,
        ownership = bullets(&task.meta.ownership),
        acceptance = bullets(&task.meta.acceptance),
        verification = bullets(&task.meta.verification),
        body = task.body
    )
}

fn collect_artifact_sections(root: &Path, sections: &mut Vec<String>) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !is_handoff_artifact(&path) {
                continue;
            }
            let body = fs::read_to_string(&path).unwrap_or_default();
            sections.push(format!("## {}\n\n{}", path.display(), body.trim()));
        }
    }
    sections.sort();
    Ok(())
}

fn is_handoff_artifact(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "txt" | "json" | "kohakutr")
    )
}

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn first_non_empty<'a>(left: &'a str, right: &'a str) -> &'a str {
    if left.trim().is_empty() { right } else { left }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::TaskMetadata;

    #[derive(Default)]
    struct FakeRunner {
        outputs: HashMap<String, Result<CommandOutput, String>>,
    }

    impl FakeRunner {
        fn with(
            mut self,
            program: &str,
            args: &[&str],
            output: Result<CommandOutput, String>,
        ) -> Self {
            self.outputs
                .insert(format!("{} {}", program, args.join(" ")), output);
            self
        }
    }

    impl RuntimeCommandRunner for FakeRunner {
        fn output(
            &self,
            program: &str,
            args: &[&str],
            _cwd: Option<&Path>,
        ) -> Result<CommandOutput, String> {
            self.outputs
                .get(&format!("{} {}", program, args.join(" ")))
                .cloned()
                .unwrap_or_else(|| Err(format!("{program} not found on PATH")))
        }
    }

    #[test]
    fn parses_runtime_kind() {
        assert_eq!(RuntimeKind::parse("claude").unwrap(), RuntimeKind::Claude);
        assert_eq!(RuntimeKind::parse("fsc").unwrap(), RuntimeKind::Fsc);
        assert_eq!(RuntimeKind::parse("kt").unwrap(), RuntimeKind::Kohaku);
    }

    #[test]
    fn kohaku_missing_cli_has_clear_error() {
        let report =
            runtime_doctor_with_runner(Path::new("."), RuntimeKind::Kohaku, &FakeRunner::default());
        assert!(!report.ok);
        assert!(report.messages[0].contains("Install KohakuTerrarium"));
    }

    #[test]
    fn kohaku_package_preserves_root_and_creature_boundaries() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        let runner = kohaku_runner();
        let task = task_card();
        let mission = Mission {
            id: task.meta.id.clone(),
            team_name: "local-1".to_string(),
            prompt: "mission".to_string(),
        };
        let opts = StartOptions {
            runtime: RuntimeKind::Kohaku,
            ..StartOptions::new(crate::MissionKind::Implementation)
        };
        let mission_path = root.join(".omc/team/missions/local-1.md");
        fs::create_dir_all(mission_path.parent().unwrap()).unwrap();
        fs::write(&mission_path, "mission").unwrap();

        let report =
            start_runtime_with_runner(&root, &mission, &task, &opts, None, &mission_path, &runner)
                .unwrap();
        let terrarium = fs::read_to_string(
            report
                .artifact_path
                .join("terrariums/omc_team/terrarium.yaml"),
        )
        .unwrap();
        assert!(terrarium.contains("root:"));
        assert!(terrarium.contains("output_wiring: [reviewer]"));
        assert!(terrarium.contains("tasks:"));

        let root_prompt = fs::read_to_string(
            report
                .artifact_path
                .join("terrariums/omc_team/prompts/root.md"),
        )
        .unwrap();
        assert!(root_prompt.contains("root creature outside the terrarium"));
        assert!(root_prompt.contains("Do not modify KohakuTerrarium upstream"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kohaku_prompts_do_not_inline_tool_docs_or_syntax() {
        let prompt = render_kohaku_creature_prompt(
            &kohaku_creatures(3)[0],
            &task_card(),
            Path::new(".omc/team/missions/local-1.md"),
        );
        assert!(!prompt.contains("[/send_message]"));
        assert!(!prompt.contains("Tool list"));
        assert!(prompt.contains("Do not manually document tools"));
    }

    #[test]
    fn fsc_missing_root_has_clear_error() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        let runner = FakeRunner::default().with(
            "bun",
            &["--version"],
            Ok(CommandOutput {
                success: true,
                stdout: "1.0.0".to_string(),
                stderr: String::new(),
            }),
        );
        let report = runtime_doctor_with_runner(&root, RuntimeKind::Fsc, &runner);
        assert!(!report.ok);
        assert!(report.messages[0].contains("FSC root not found"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_handoff_collects_kohaku_artifacts() {
        let root = unique_temp_dir();
        let artifact = root.join(".omc/team/kohaku/run-1");
        fs::create_dir_all(&artifact).unwrap();
        fs::write(artifact.join("handoff.md"), "Done\n").unwrap();
        fs::write(artifact.join("session.kohakutr"), "session-index\n").unwrap();
        save_runtime_record(
            &root,
            &RuntimeRunRecord {
                record_type: "runtime".to_string(),
                run_id: "run-1".to_string(),
                runtime: RuntimeKind::Kohaku,
                team_name: "team-1".to_string(),
                task_id: "LOCAL-1".to_string(),
                mission_path: "mission.md".to_string(),
                artifact_path: artifact.display().to_string(),
                started_at: 1,
                launch_command: vec![],
                tracker: None,
                tracker_run_id: None,
                tracker_team_name: None,
                status: "prepared".to_string(),
            },
        )
        .unwrap();

        let summary = collect_runtime_handoff(&root, "run-1", RuntimeKind::Kohaku).unwrap();
        assert!(summary.contains("Done"));
        assert!(summary.contains("session.kohakutr"));
        let _ = fs::remove_dir_all(root);
    }

    fn kohaku_runner() -> FakeRunner {
        FakeRunner::default()
            .with(
                "kt",
                &["--version"],
                Ok(CommandOutput {
                    success: true,
                    stdout: "kt 1.3.0".to_string(),
                    stderr: String::new(),
                }),
            )
            .with(
                "kt",
                &["list"],
                Ok(CommandOutput {
                    success: true,
                    stdout: "kt-biome".to_string(),
                    stderr: String::new(),
                }),
            )
    }

    fn task_card() -> TaskCard {
        TaskCard {
            meta: TaskMetadata {
                id: "LOCAL-1".to_string(),
                title: "Build runtime adapter".to_string(),
                agent_ready: true,
                risk: "medium".to_string(),
                ownership: vec!["crates/omc-team".to_string()],
                acceptance: vec!["runtime starts".to_string()],
                verification: vec!["cargo test -p omc-team".to_string()],
                source: None,
                linear_id: None,
                tracker: None,
                github_repo: None,
                github_issue_number: None,
            },
            body: "Implement the adapter.".to_string(),
        }
    }

    fn unique_temp_dir() -> PathBuf {
        let mut dir = env::temp_dir();
        dir.push(format!(
            "omc-runtime-test-{}",
            crate::unix_timestamp_nanos()
        ));
        dir
    }
}
