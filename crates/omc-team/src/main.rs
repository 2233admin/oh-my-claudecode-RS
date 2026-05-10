use std::env;
use std::io::{self, Read};

use omc_team::{
    ClaimReport, DoctorReport, HookKind, InitReport, Mission, MissionKind,
    ObservabilityDoctorReport, ObservabilityStartReport, ObservabilityTopSnapshot, ReadyIssue,
    RuntimeDoctorReport, RuntimeKind, StartOptions, TaskCard, TaskMetadata, TrackerKind,
    UsageGroupBy, UsageReport, check_claude_ready, check_runtime_ready, collect_handoff,
    collect_runtime_handoff, finalize_start_record, github_claim, github_doctor,
    github_import_task, github_ready, handle_hook, import_linear_issue, init_project, linear_claim,
    linear_doctor, linear_ready, mission_for_research, mission_for_review, new_run_id,
    observability_doctor, post_handoff_to_tracker, post_runtime_handoff_to_tracker,
    prepare_start_mission, record_team_launch, render_resume_packet, runtime_doctor, start_runtime,
    top_snapshot, usage_report, write_mission,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("omc-team: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        print_help();
        return Ok(());
    };
    args.remove(0);

    match cmd.as_str() {
        "init" => {
            let report = init_project(&env::current_dir().map_err(|e| e.to_string())?)?;
            print_init_report(&report);
        }
        "start" => {
            let target = take_required(&mut args, "task-or-issue")?;
            let opts = parse_start_options(args, MissionKind::Implementation)?;
            check_runtime_ready(
                &env::current_dir().map_err(|e| e.to_string())?,
                opts.runtime,
            )?;
            let root = env::current_dir().map_err(|e| e.to_string())?;
            let prepared = prepare_start_mission(&root, &target, opts.clone())?;
            let path = write_mission(&root, &prepared.mission)?;
            let tracker_record = if let Some(record) = prepared.run_record {
                let record = finalize_start_record(&root, record, &path)?;
                println!(
                    "Tracker started: {} {} ({})",
                    record.tracker.as_str(),
                    record.issue_ref,
                    record.run_id
                );
                Some(record)
            } else {
                None
            };
            if opts.runtime == RuntimeKind::Claude {
                let run_id = tracker_record.as_ref().map_or_else(
                    || new_run_id(&prepared.mission.team_name),
                    |record| record.run_id.clone(),
                );
                let observability = record_team_launch(
                    &root,
                    &run_id,
                    &prepared.mission,
                    &prepared.task,
                    &opts,
                    &path,
                    tracker_record.as_ref().map(|record| record.run_id.as_str()),
                )?;
                print_observability_start(&observability);
                println!("{}", prepared.mission.prompt);
            } else {
                let report = start_runtime(
                    &root,
                    &prepared.mission,
                    &prepared.task,
                    &opts,
                    tracker_record.as_ref(),
                    &path,
                )?;
                let observability = record_team_launch(
                    &root,
                    &report.run_id,
                    &prepared.mission,
                    &prepared.task,
                    &opts,
                    &path,
                    tracker_record.as_ref().map(|record| record.run_id.as_str()),
                )?;
                print_runtime_start(&report);
                print_observability_start(&observability);
            }
            println!("\nMission written: {}", path.display());
        }
        "research" => {
            let topic = take_required(&mut args, "topic")?;
            let opts = parse_start_options(args, MissionKind::Research)?;
            check_claude_ready()?;
            let mission = mission_for_research(&topic, opts.clone());
            let root = env::current_dir().map_err(|e| e.to_string())?;
            let path = write_mission(&root, &mission)?;
            let task = synthetic_task_from_mission(&mission, "research", &topic);
            let run_id = new_run_id(&mission.team_name);
            let observability =
                record_team_launch(&root, &run_id, &mission, &task, &opts, &path, None)?;
            print_observability_start(&observability);
            println!("{}", mission.prompt);
            println!("\nMission written: {}", path.display());
        }
        "review" => {
            let target = take_required(&mut args, "pr-or-branch")?;
            let opts = parse_review_options(args)?;
            check_claude_ready()?;
            let mission = mission_for_review(&target, opts.clone());
            let root = env::current_dir().map_err(|e| e.to_string())?;
            let path = write_mission(&root, &mission)?;
            let task = synthetic_task_from_mission(&mission, "review", &target);
            let run_id = new_run_id(&mission.team_name);
            let observability =
                record_team_launch(&root, &run_id, &mission, &task, &opts, &path, None)?;
            print_observability_start(&observability);
            println!("{}", mission.prompt);
            println!("\nMission written: {}", path.display());
        }
        "session" => {
            let action = take_required(&mut args, "session-action")?;
            let root = env::current_dir().map_err(|e| e.to_string())?;
            match action.as_str() {
                "list" => {
                    if !args.is_empty() {
                        return Err(format!("unknown option: {}", args.join(" ")));
                    }
                    let sessions = omc_team::load_sessions(&root)?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&sessions).map_err(|e| e.to_string())?
                    );
                }
                "resume" => {
                    let target = take_required(&mut args, "agent-id-or-run-id")?;
                    if !args.is_empty() {
                        return Err(format!("unknown option: {}", args.join(" ")));
                    }
                    let packet = render_resume_packet(&root, &target)?;
                    println!("{packet}");
                }
                _ => return Err(format!("unknown session action: {action}")),
            }
        }
        "usage" => {
            let action = take_required(&mut args, "usage-action")?;
            let root = env::current_dir().map_err(|e| e.to_string())?;
            match action.as_str() {
                "report" => {
                    let opts = parse_usage_options(args)?;
                    let report = usage_report(&root, opts.run_id.as_deref(), opts.group_by)?;
                    print_usage_report(&report)?;
                }
                _ => return Err(format!("unknown usage action: {action}")),
            }
        }
        "top" => {
            if !args.is_empty() {
                return Err(format!("unknown option: {}", args.join(" ")));
            }
            let root = env::current_dir().map_err(|e| e.to_string())?;
            let snapshot = top_snapshot(&root)?;
            print_top_snapshot(&snapshot)?;
        }
        "doctor" => {
            let action = take_required(&mut args, "doctor-action")?;
            match action.as_str() {
                "observability" => {
                    if !args.is_empty() {
                        return Err(format!("unknown option: {}", args.join(" ")));
                    }
                    let root = env::current_dir().map_err(|e| e.to_string())?;
                    print_observability_doctor(&observability_doctor(&root))?;
                }
                _ => return Err(format!("unknown doctor action: {action}")),
            }
        }
        "linear" => {
            let action = take_required(&mut args, "linear-action")?;
            match action.as_str() {
                "doctor" => {
                    let opts = parse_tracker_options(args)?;
                    let report = linear_doctor(opts.team.as_deref(), opts.fix)?;
                    print_doctor(&report)?;
                }
                "import" => {
                    let issue = take_required(&mut args, "issue-id")?;
                    let path = import_linear_issue(
                        &env::current_dir().map_err(|e| e.to_string())?,
                        &issue,
                    )?;
                    println!("Imported Linear issue {issue} to {}", path.display());
                }
                "ready" => {
                    let opts = parse_tracker_options(args)?;
                    let issues = linear_ready(opts.team.as_deref(), opts.limit)?;
                    print_ready(&issues)?;
                }
                "claim" => {
                    let opts = parse_tracker_options(args)?;
                    let claim = linear_claim(opts.team.as_deref(), opts.limit)?;
                    print_claim(&claim)?;
                }
                _ => return Err(format!("unknown linear action: {action}")),
            }
        }
        "github" | "gh" => {
            let action = take_required(&mut args, "github-action")?;
            match action.as_str() {
                "doctor" => {
                    let opts = parse_tracker_options(args)?;
                    let root = env::current_dir().map_err(|e| e.to_string())?;
                    let report = github_doctor(&root, opts.repo.as_deref(), opts.fix)?;
                    print_doctor(&report)?;
                }
                "import" => {
                    let issue = take_required(&mut args, "issue-ref")?;
                    let opts = parse_tracker_options(args)?;
                    let root = env::current_dir().map_err(|e| e.to_string())?;
                    let imported = github_import_task(&root, &issue, opts.repo.as_deref())?;
                    println!(
                        "Imported GitHub issue {} ({})",
                        imported.issue_ref, imported.card.meta.title
                    );
                }
                "ready" => {
                    let opts = parse_tracker_options(args)?;
                    let root = env::current_dir().map_err(|e| e.to_string())?;
                    let issues = github_ready(&root, opts.repo.as_deref(), opts.limit)?;
                    print_ready(&issues)?;
                }
                "claim" => {
                    let opts = parse_tracker_options(args)?;
                    let root = env::current_dir().map_err(|e| e.to_string())?;
                    let claim = github_claim(&root, opts.repo.as_deref(), opts.limit)?;
                    print_claim(&claim)?;
                }
                _ => return Err(format!("unknown github action: {action}")),
            }
        }
        "runtime" => {
            let action = take_required(&mut args, "runtime-action")?;
            match action.as_str() {
                "doctor" => {
                    let runtime = RuntimeKind::parse(&take_required(&mut args, "runtime")?)?;
                    if !args.is_empty() {
                        return Err(format!("unknown option: {}", args.join(" ")));
                    }
                    let root = env::current_dir().map_err(|e| e.to_string())?;
                    let report = runtime_doctor(&root, runtime);
                    print_runtime_doctor(&report)?;
                }
                _ => return Err(format!("unknown runtime action: {action}")),
            }
        }
        "handoff" => {
            let team_name = take_required(&mut args, "team-name")?;
            let handoff = parse_handoff_options(args)?;
            let root = env::current_dir().map_err(|e| e.to_string())?;
            let summary = if let Some(runtime) = handoff.runtime {
                collect_runtime_handoff(&root, &team_name, runtime)?
            } else {
                collect_handoff(&root, &team_name)?
            };
            if let Some(tracker) = handoff.tracker {
                let record = if let Some(runtime) = handoff.runtime {
                    post_runtime_handoff_to_tracker(
                        &root,
                        &team_name,
                        &summary,
                        runtime,
                        tracker,
                        handoff.done,
                    )?
                } else {
                    post_handoff_to_tracker(&root, &team_name, &summary, tracker, handoff.done)?
                };
                println!(
                    "Tracker handoff posted: {} {} ({})",
                    record.tracker.as_str(),
                    record.issue_ref,
                    record.last_known_state.as_deref().unwrap_or("handoff")
                );
            }
            println!("{summary}");
        }
        "hook" => {
            let hook = take_required(&mut args, "hook-kind")?;
            let kind = HookKind::parse(&hook)?;
            let mut stdin = String::new();
            io::stdin()
                .read_to_string(&mut stdin)
                .map_err(|e| format!("failed to read hook stdin: {e}"))?;
            match handle_hook(kind, &stdin) {
                Ok(()) => {}
                Err(message) => {
                    eprintln!("{message}");
                    std::process::exit(2);
                }
            }
        }
        "help" | "--help" | "-h" => print_help(),
        _ => return Err(format!("unknown command: {cmd}")),
    }

    Ok(())
}

fn take_required(args: &mut Vec<String>, name: &str) -> Result<String, String> {
    if args.is_empty() {
        return Err(format!("missing {name}"));
    }
    Ok(args.remove(0))
}

fn parse_start_options(args: Vec<String>, kind: MissionKind) -> Result<StartOptions, String> {
    let mut opts = StartOptions::new(kind);
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--team-size" => {
                let raw = iter.next().ok_or("missing --team-size value")?;
                opts.team_size = raw
                    .parse::<u8>()
                    .map_err(|_| format!("invalid team size: {raw}"))?;
            }
            "--mode" => {
                opts.mode = iter.next().ok_or("missing --mode value")?;
            }
            "--repo" => {
                opts.repo = Some(iter.next().ok_or("missing --repo value")?);
            }
            "--team" => {
                opts.team = Some(iter.next().ok_or("missing --team value")?);
            }
            "--tracker" => {
                let raw = iter.next().ok_or("missing --tracker value")?;
                opts.tracker = Some(TrackerKind::parse(&raw)?);
            }
            "--runtime" => {
                let raw = iter.next().ok_or("missing --runtime value")?;
                opts.runtime = RuntimeKind::parse(&raw)?;
            }
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    opts.validate()?;
    Ok(opts)
}

fn parse_review_options(args: Vec<String>) -> Result<StartOptions, String> {
    let mut opts = StartOptions::new(MissionKind::Review);
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--team-size" => {
                let raw = iter.next().ok_or("missing --team-size value")?;
                opts.team_size = raw
                    .parse::<u8>()
                    .map_err(|_| format!("invalid team size: {raw}"))?;
            }
            "--security" => opts.security_review = true,
            "--tests" => opts.test_review = true,
            "--repo" => {
                opts.repo = Some(iter.next().ok_or("missing --repo value")?);
            }
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    opts.validate()?;
    Ok(opts)
}

#[derive(Debug, Default)]
struct TrackerCliOptions {
    repo: Option<String>,
    team: Option<String>,
    limit: usize,
    fix: bool,
}

fn parse_tracker_options(args: Vec<String>) -> Result<TrackerCliOptions, String> {
    let mut opts = TrackerCliOptions {
        limit: 20,
        ..TrackerCliOptions::default()
    };
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => opts.repo = Some(iter.next().ok_or("missing --repo value")?),
            "--team" => opts.team = Some(iter.next().ok_or("missing --team value")?),
            "--limit" => {
                let raw = iter.next().ok_or("missing --limit value")?;
                opts.limit = raw
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --limit value: {raw}"))?;
            }
            "--fix" => opts.fix = true,
            _ => return Err(format!("unknown option: {arg}")),
        }
    }
    Ok(opts)
}

#[derive(Debug, Default)]
struct HandoffOptions {
    tracker: Option<TrackerKind>,
    runtime: Option<RuntimeKind>,
    done: bool,
}

fn parse_handoff_options(args: Vec<String>) -> Result<HandoffOptions, String> {
    let mut opts = HandoffOptions::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--github" | "--gh" => opts.tracker = Some(TrackerKind::GitHub),
            "--linear" => opts.tracker = Some(TrackerKind::Linear),
            "--runtime" => {
                let raw = iter.next().ok_or("missing --runtime value")?;
                opts.runtime = Some(RuntimeKind::parse(&raw)?);
            }
            "--done" => opts.done = true,
            _ => return Err(format!("unknown handoff option: {arg}")),
        }
    }
    Ok(opts)
}

#[derive(Debug)]
struct UsageOptions {
    run_id: Option<String>,
    group_by: UsageGroupBy,
}

fn parse_usage_options(args: Vec<String>) -> Result<UsageOptions, String> {
    let mut opts = UsageOptions {
        run_id: None,
        group_by: UsageGroupBy::Agent,
    };
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--run" => opts.run_id = Some(iter.next().ok_or("missing --run value")?),
            "--by" => {
                let raw = iter.next().ok_or("missing --by value")?;
                opts.group_by = UsageGroupBy::parse(&raw)?;
            }
            _ => return Err(format!("unknown usage option: {arg}")),
        }
    }
    Ok(opts)
}

fn print_doctor(report: &DoctorReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_ready(issues: &[ReadyIssue]) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(issues).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_claim(claim: &ClaimReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(claim).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_init_report(report: &InitReport) {
    println!("omc-team initialized");
    for path in &report.created {
        println!("  created: {}", path.display());
    }
    for path in &report.updated {
        println!("  updated: {}", path.display());
    }
    for path in &report.unchanged {
        println!("  unchanged: {}", path.display());
    }
}

fn print_runtime_doctor(report: &RuntimeDoctorReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_runtime_start(report: &omc_team::RuntimeStartReport) {
    println!(
        "Runtime prepared: {} {} ({})",
        report.runtime.as_str(),
        report.team_name,
        report.run_id
    );
    println!("Artifacts: {}", report.artifact_path.display());
    println!("Launch command:");
    println!("  {}", report.launch_command.join(" "));
}

fn print_observability_start(report: &ObservabilityStartReport) {
    println!(
        "Observability: {} sessions, {} invocations, briefing {}",
        report.session_count,
        report.invocation_count,
        report.briefing_path.display()
    );
}

fn print_usage_report(report: &UsageReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_top_snapshot(snapshot: &ObservabilityTopSnapshot) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn print_observability_doctor(report: &ObservabilityDoctorReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn synthetic_task_from_mission(mission: &Mission, kind: &str, target: &str) -> TaskCard {
    TaskCard {
        meta: TaskMetadata {
            id: mission.id.clone(),
            title: format!("{kind}: {target}"),
            agent_ready: true,
            risk: "medium".to_string(),
            ownership: vec![format!("{kind} mission")],
            acceptance: vec![format!("complete {kind} mission for {target}")],
            verification: vec![
                "lead handoff includes evidence, risks, and next action".to_string(),
            ],
            source: Some("local".to_string()),
            linear_id: None,
            tracker: None,
            github_repo: None,
            github_issue_number: None,
        },
        body: mission.prompt.clone(),
    }
}

fn print_help() {
    println!(
        "omc-team - Claude Code experimental agent teams orchestration shell\n\n\
Usage:\n  omc-team init\n  omc-team start <task-or-issue> [--runtime claude|fsc|kohaku] [--tracker github|linear] [--repo owner/name] [--team TEAM] [--team-size N]\n  omc-team research <topic> [--team-size N]\n  omc-team review <pr-or-branch> [--team-size N] [--security] [--tests]\n  omc-team runtime doctor claude|fsc|kohaku\n  omc-team linear doctor [--team TEAM] [--fix]\n  omc-team linear import <issue-id>\n  omc-team linear ready [--team TEAM] [--limit N]\n  omc-team linear claim [--team TEAM]\n  omc-team github doctor [--repo owner/name] [--fix]\n  omc-team github import <#123|owner/repo#123|url> [--repo owner/name]\n  omc-team github ready [--repo owner/name] [--limit N]\n  omc-team github claim [--repo owner/name]\n  omc-team session list\n  omc-team session resume <agent-id-or-run-id>\n  omc-team usage report [--run RUN_ID] [--by agent|cell|runtime|provider]\n  omc-team top\n  omc-team doctor observability\n  omc-team handoff <team-name-or-run-id> [--runtime fsc|kohaku] [--github|--linear] [--done]\n  omc-team hook <task-created|task-completed|teammate-idle>\n"
    );
}
