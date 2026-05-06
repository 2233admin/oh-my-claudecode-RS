use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{TaskCard, TaskMetadata, render_task_card, slug, unix_timestamp};

const AGENT_READY: &str = "agent-ready";
const GITHUB_CLAIMED: &str = "omc/claimed";
const GITHUB_IN_PROGRESS: &str = "omc/in-progress";
const GITHUB_IN_REVIEW: &str = "omc/in-review";
const GITHUB_DONE: &str = "omc/done";
const LEASE_PREFIX: &str = "<!-- omc-team-lease ";
const RUN_PREFIX: &str = "<!-- omc-team-run ";
const COMMENT_END: &str = "-->";
const LEASE_TTL_SECONDS: u64 = 60 * 60 * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackerKind {
    Linear,
    GitHub,
}

impl TrackerKind {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.to_ascii_lowercase().as_str() {
            "linear" => Ok(Self::Linear),
            "github" | "gh" => Ok(Self::GitHub),
            _ => Err(format!("unknown tracker: {raw}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::GitHub => "github",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub tracker: TrackerKind,
    pub repo_or_team: String,
    pub issue_ref: String,
    pub issue_id: String,
    pub team_name: String,
    pub mission_path: String,
    pub started_at: u64,
    pub lease_comment_id: Option<String>,
    pub start_comment_id: Option<String>,
    pub handoff_comment_id: Option<String>,
    pub last_known_state: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub tracker: TrackerKind,
    pub target: String,
    pub ok: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadyIssue {
    pub tracker: TrackerKind,
    pub repo_or_team: String,
    pub issue_ref: String,
    pub title: String,
    pub state: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimReport {
    pub tracker: TrackerKind,
    pub repo_or_team: String,
    pub issue_ref: String,
    pub title: String,
    pub run_id: String,
    pub lease_comment_id: String,
}

#[derive(Debug, Clone)]
pub struct ImportedTask {
    pub card: TaskCard,
    pub tracker: TrackerKind,
    pub repo_or_team: String,
    pub issue_ref: String,
    pub issue_id: String,
}

#[derive(Debug, Clone)]
struct TrackerComment {
    id: String,
    body: String,
    created_at: String,
}

#[derive(Debug, Clone)]
struct Lease {
    run_id: String,
    expires_at: u64,
}

#[derive(Debug, Clone)]
struct ExternalIssue {
    id: String,
    issue_ref: String,
    title: String,
    body: String,
    state: String,
    labels: Vec<String>,
    number: Option<u64>,
    team_name: Option<String>,
    comments: Vec<TrackerComment>,
}

#[derive(Debug, Clone)]
struct GitHubApi {
    repo: String,
    token: String,
}

#[derive(Debug, Clone)]
struct LinearApi {
    token: String,
}

pub fn looks_like_github_issue_ref(raw: &str) -> bool {
    let raw = raw.trim();
    raw.starts_with('#')
        || raw.contains("github.com/")
        || (raw.contains('#') && raw.split('#').next_back().is_some_and(is_digits))
}

pub fn import_tracker_task(
    root: &Path,
    target: &str,
    tracker: TrackerKind,
    repo: Option<&str>,
    team: Option<&str>,
) -> Result<ImportedTask, String> {
    match tracker {
        TrackerKind::GitHub => github_import_task(root, target, repo),
        TrackerKind::Linear => linear_import_task(root, target, team),
    }
}

pub fn claim_specific_issue(imported: &ImportedTask, run_id: &str) -> Result<String, String> {
    match imported.tracker {
        TrackerKind::GitHub => {
            let number = imported
                .issue_id
                .parse::<u64>()
                .map_err(|_| format!("invalid GitHub issue number: {}", imported.issue_id))?;
            github_claim_issue(&imported.repo_or_team, number, run_id)
        }
        TrackerKind::Linear => linear_claim_issue(&imported.issue_id, run_id),
    }
}

pub fn mark_started(record: &RunRecord) -> Result<Option<String>, String> {
    match record.tracker {
        TrackerKind::GitHub => github_mark_started(record),
        TrackerKind::Linear => linear_mark_started(record),
    }
}

pub fn post_handoff(
    record: &RunRecord,
    summary: &str,
    done: bool,
) -> Result<Option<String>, String> {
    match record.tracker {
        TrackerKind::GitHub => github_post_handoff(record, summary, done),
        TrackerKind::Linear => linear_post_handoff(record, summary, done),
    }
}

pub fn save_run_record(root: &Path, record: &RunRecord) -> Result<(), String> {
    let dir = root.join(".omc/team/runs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", record.run_id));
    let rendered = serde_json::to_string_pretty(record).map_err(|e| e.to_string())? + "\n";
    fs::write(path, rendered).map_err(|e| e.to_string())
}

pub fn find_run_record(
    root: &Path,
    team_name: &str,
    tracker: Option<TrackerKind>,
) -> Result<RunRecord, String> {
    let mut matches = load_tracker_run_records(root)?
        .into_iter()
        .filter(|record| {
            record.team_name == team_name && tracker.is_none_or(|kind| kind == record.tracker)
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|record| record.started_at);
    matches
        .pop()
        .ok_or_else(|| format!("no run record found for team {team_name}"))
}

pub fn find_run_record_by_run_id(
    root: &Path,
    run_id: &str,
    tracker: Option<TrackerKind>,
) -> Result<RunRecord, String> {
    let mut matches = load_tracker_run_records(root)?
        .into_iter()
        .filter(|record| {
            record.run_id == run_id && tracker.is_none_or(|kind| kind == record.tracker)
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|record| record.started_at);
    matches
        .pop()
        .ok_or_else(|| format!("no run record found for run {run_id}"))
}

fn load_tracker_run_records(root: &Path) -> Result<Vec<RunRecord>, String> {
    let dir = root.join(".omc/team/runs");
    let mut records = Vec::new();
    if !dir.exists() {
        return Ok(records);
    }
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&raw)
            .map_err(|e| format!("invalid run record {}: {e}", path.display()))?;
        if value.get("tracker").is_none() {
            continue;
        }
        let record: RunRecord = serde_json::from_value(value)
            .map_err(|e| format!("invalid tracker run record {}: {e}", path.display()))?;
        records.push(record);
    }
    Ok(records)
}

pub fn github_doctor(root: &Path, repo: Option<&str>, fix: bool) -> Result<DoctorReport, String> {
    let api = github_api(root, repo)?;
    github_request(&api, "GET", &format!("/repos/{}", api.repo), None)?;
    let labels = github_label_set(&api)?;
    let mut messages = vec![format!("GitHub repo ok: {}", api.repo)];
    let mut ok = true;
    for &label in github_required_labels() {
        if labels.contains(label) {
            messages.push(format!("label ok: {label}"));
        } else if fix {
            github_create_label(&api, label)?;
            messages.push(format!("created label: {label}"));
        } else {
            ok = false;
            messages.push(format!("missing label: {label}"));
        }
    }
    Ok(DoctorReport {
        tracker: TrackerKind::GitHub,
        target: api.repo,
        ok,
        messages,
    })
}

pub fn github_import_task(
    root: &Path,
    issue_ref: &str,
    repo: Option<&str>,
) -> Result<ImportedTask, String> {
    let api = github_api(root, repo)?;
    let parsed = parse_github_issue_ref(issue_ref, Some(&api.repo))?;
    let api = GitHubApi {
        repo: parsed.repo,
        token: api.token,
    };
    let issue = github_get_issue(&api, parsed.number, true)?;
    let card = github_issue_to_card(&api.repo, &issue);
    let path = write_import(root, &issue.issue_ref, &card)?;
    let _ = path;
    Ok(ImportedTask {
        card,
        tracker: TrackerKind::GitHub,
        repo_or_team: api.repo,
        issue_ref: issue.issue_ref,
        issue_id: parsed.number.to_string(),
    })
}

pub fn github_ready(
    root: &Path,
    repo: Option<&str>,
    limit: usize,
) -> Result<Vec<ReadyIssue>, String> {
    let api = github_api(root, repo)?;
    github_ready_with_api(&api, limit)
}

pub fn github_claim(root: &Path, repo: Option<&str>, limit: usize) -> Result<ClaimReport, String> {
    let api = github_api(root, repo)?;
    let Some(issue) = github_ready_with_api(&api, limit)?.into_iter().next() else {
        return Err(format!(
            "no {AGENT_READY} GitHub issues found in {}",
            api.repo
        ));
    };
    let parsed = parse_github_issue_ref(&issue.issue_ref, Some(&api.repo))?;
    let run_id = new_run_id(&issue.issue_ref);
    let lease_comment_id = github_claim_issue(&api.repo, parsed.number, &run_id)?;
    Ok(ClaimReport {
        tracker: TrackerKind::GitHub,
        repo_or_team: api.repo,
        issue_ref: issue.issue_ref,
        title: issue.title,
        run_id,
        lease_comment_id,
    })
}

pub fn linear_doctor(team: Option<&str>, fix: bool) -> Result<DoctorReport, String> {
    let api = linear_api()?;
    let team_name = team.unwrap_or("OMC-RS");
    let team_value = linear_find_team(&api, team_name)?;
    let target = team_value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(team_name)
        .to_string();
    let states = linear_team_states(&api, team_value["id"].as_str().unwrap_or_default())?;
    let labels = linear_team_labels(&api, team_value["id"].as_str().unwrap_or_default())?;
    let mut messages = vec![format!("Linear team ok: {target}")];
    let mut ok = true;
    for state in ["In Progress", "In Review", "Done"] {
        if states.contains(state) {
            messages.push(format!("state ok: {state}"));
        } else {
            messages.push(format!("missing state; will comment only: {state}"));
        }
    }
    if labels.contains(AGENT_READY) {
        messages.push(format!("label ok: {AGENT_READY}"));
    } else if fix {
        linear_create_label(
            &api,
            team_value["id"].as_str().unwrap_or_default(),
            AGENT_READY,
        )?;
        messages.push(format!("created label: {AGENT_READY}"));
    } else {
        ok = false;
        messages.push(format!("missing label: {AGENT_READY}"));
    }
    Ok(DoctorReport {
        tracker: TrackerKind::Linear,
        target,
        ok,
        messages,
    })
}

pub fn linear_import_task(
    root: &Path,
    issue_ref: &str,
    _team: Option<&str>,
) -> Result<ImportedTask, String> {
    let api = linear_api()?;
    let issue = linear_find_issue(&api, issue_ref)?;
    let card = linear_issue_to_card(&issue);
    let path = write_import(root, &issue.issue_ref, &card)?;
    let _ = path;
    Ok(ImportedTask {
        card,
        tracker: TrackerKind::Linear,
        repo_or_team: issue.team_name.unwrap_or_else(|| "Linear".to_string()),
        issue_ref: issue.issue_ref,
        issue_id: issue.id,
    })
}

pub fn linear_ready(team: Option<&str>, limit: usize) -> Result<Vec<ReadyIssue>, String> {
    let api = linear_api()?;
    let team_name = team.unwrap_or("OMC-RS");
    let team_value = linear_find_team(&api, team_name)?;
    let team_id = team_value["id"].as_str().unwrap_or_default();
    let issues = linear_team_issues(&api, team_id, usize::MAX)?;
    let now = unix_timestamp();
    Ok(issues
        .into_iter()
        .filter(|issue| is_ready_labels(&issue.labels))
        .filter(|issue| !linear_progress_state(&issue.state))
        .filter(|issue| earliest_active_lease(&issue.comments, now).is_none())
        .take(limit)
        .map(|issue| ReadyIssue {
            tracker: TrackerKind::Linear,
            repo_or_team: issue.team_name.unwrap_or_else(|| team_name.to_string()),
            issue_ref: issue.issue_ref,
            title: issue.title,
            state: issue.state,
            labels: issue.labels,
        })
        .collect())
}

pub fn linear_claim(team: Option<&str>, limit: usize) -> Result<ClaimReport, String> {
    let Some(issue) = linear_ready(team, limit)?.into_iter().next() else {
        return Err(format!(
            "no {AGENT_READY} Linear issues found in {}",
            team.unwrap_or("OMC-RS")
        ));
    };
    let api = linear_api()?;
    let external = linear_find_issue(&api, &issue.issue_ref)?;
    let run_id = new_run_id(&issue.issue_ref);
    let lease_comment_id = linear_claim_issue(&external.id, &run_id)?;
    Ok(ClaimReport {
        tracker: TrackerKind::Linear,
        repo_or_team: issue.repo_or_team,
        issue_ref: issue.issue_ref,
        title: issue.title,
        run_id,
        lease_comment_id,
    })
}

fn write_import(
    root: &Path,
    issue_ref: &str,
    card: &TaskCard,
) -> Result<std::path::PathBuf, String> {
    let dir = root.join(".omc/team/imports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.md", slug(issue_ref)));
    fs::write(&path, render_task_card(card)).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn new_run_id(issue_ref: &str) -> String {
    format!("omc-{}-{}", slug(issue_ref), unix_timestamp())
}

fn github_api(root: &Path, repo: Option<&str>) -> Result<GitHubApi, String> {
    let token = github_token()?;
    let repo = detect_github_repo(root, repo)?;
    Ok(GitHubApi { repo, token })
}

fn github_token() -> Result<String, String> {
    if let Ok(token) = env::var("GITHUB_TOKEN")
        && !token.trim().is_empty()
    {
        return Ok(token);
    }
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|_| {
            "GitHub token missing. Run `gh auth login` or set GITHUB_TOKEN.".to_string()
        })?;
    if !output.status.success() {
        return Err("GitHub token missing. Run `gh auth login` or set GITHUB_TOKEN.".to_string());
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        Err("GitHub token missing. Run `gh auth login` or set GITHUB_TOKEN.".to_string())
    } else {
        Ok(token)
    }
}

pub(crate) fn detect_github_repo(root: &Path, explicit: Option<&str>) -> Result<String, String> {
    if let Some(repo) = explicit {
        return validate_repo(repo);
    }
    if let Ok(repo) = env::var("GITHUB_REPOSITORY")
        && !repo.trim().is_empty()
    {
        return validate_repo(&repo);
    }
    let output = Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "remote",
            "get-url",
            "origin",
        ])
        .output()
        .map_err(|_| "cannot detect GitHub repo; pass --repo owner/name".to_string())?;
    if !output.status.success() {
        return Err("cannot detect GitHub repo; pass --repo owner/name".to_string());
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_github_remote(&raw)
        .ok_or_else(|| "origin is not a GitHub remote; pass --repo owner/name".to_string())
}

fn validate_repo(raw: &str) -> Result<String, String> {
    let repo = raw.trim().trim_end_matches(".git");
    let parts = repo.split('/').collect::<Vec<_>>();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Ok(repo.to_string())
    } else {
        Err(format!("invalid GitHub repo `{raw}`; expected owner/name"))
    }
}

pub(crate) fn parse_github_remote(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_end_matches(".git");
    if let Some(rest) = raw.strip_prefix("https://github.com/") {
        return validate_repo(rest).ok();
    }
    if let Some(rest) = raw.strip_prefix("http://github.com/") {
        return validate_repo(rest).ok();
    }
    if let Some(rest) = raw.strip_prefix("git@github.com:") {
        return validate_repo(rest).ok();
    }
    if let Some(rest) = raw.strip_prefix("ssh://git@github.com/") {
        return validate_repo(rest).ok();
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedGitHubIssue {
    pub repo: String,
    pub number: u64,
}

pub(crate) fn parse_github_issue_ref(
    raw: &str,
    default_repo: Option<&str>,
) -> Result<ParsedGitHubIssue, String> {
    let raw = raw.trim();
    if let Some(after_host) = raw.split("github.com/").nth(1) {
        let parts = after_host
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .split('/')
            .collect::<Vec<_>>();
        if parts.len() >= 4 && (parts[2] == "issues" || parts[2] == "pull") {
            return Ok(ParsedGitHubIssue {
                repo: validate_repo(&format!("{}/{}", parts[0], parts[1]))?,
                number: parse_issue_number(parts[3])?,
            });
        }
    }
    if let Some((repo, number)) = raw.rsplit_once('#') {
        let repo = if repo.trim().is_empty() {
            default_repo.ok_or("GitHub issue ref needs --repo owner/name")?
        } else {
            repo
        };
        return Ok(ParsedGitHubIssue {
            repo: validate_repo(repo)?,
            number: parse_issue_number(number)?,
        });
    }
    if is_digits(raw) {
        return Ok(ParsedGitHubIssue {
            repo: validate_repo(default_repo.ok_or("GitHub issue ref needs --repo owner/name")?)?,
            number: parse_issue_number(raw)?,
        });
    }
    Err(format!("invalid GitHub issue ref `{raw}`"))
}

fn parse_issue_number(raw: &str) -> Result<u64, String> {
    raw.trim()
        .trim_start_matches('#')
        .parse::<u64>()
        .map_err(|_| format!("invalid GitHub issue number: {raw}"))
}

fn github_request(
    api: &GitHubApi,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let url = format!("https://api.github.com{path}");
    http_request_json(
        method,
        &url,
        &[
            "Accept: application/vnd.github+json".to_string(),
            format!("Authorization: Bearer {}", api.token),
            "Content-Type: application/json".to_string(),
            "X-GitHub-Api-Version: 2022-11-28".to_string(),
            "User-Agent: omc-team".to_string(),
        ],
        body,
    )
}

fn http_request_json(
    method: &str,
    url: &str,
    headers: &[String],
    body: Option<Value>,
) -> Result<Value, String> {
    let marker = "OMC_HTTP_STATUS:";
    let mut args = vec![
        "-sS".to_string(),
        "-L".to_string(),
        "-X".to_string(),
        method.to_string(),
        "-w".to_string(),
        format!("\n{marker}%{{http_code}}"),
    ];
    for header in headers {
        args.push("-H".to_string());
        args.push(header.clone());
    }
    let body_path = if let Some(body) = body {
        let path = write_temp_json_body(&body)?;
        args.push("--data-binary".to_string());
        args.push(format!("@{}", path.display()));
        Some(path)
    } else {
        None
    };
    args.push(url.to_string());

    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|_| "curl not found; install curl or add it to PATH".to_string());
    if let Some(path) = body_path {
        let _ = fs::remove_file(path);
    }
    let output = output?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!("curl failed for {url}: {}", stderr.trim()));
    }
    let Some((payload, status_raw)) = stdout.rsplit_once(marker) else {
        return Err(format!(
            "curl response for {url} did not include HTTP status"
        ));
    };
    let status = status_raw.trim().parse::<u16>().unwrap_or(0);
    let payload = payload.trim();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status} from {url}: {payload}"));
    }
    if payload.is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(payload).map_err(|e| format!("invalid JSON from {url}: {e}"))
    }
}

fn write_temp_json_body(body: &Value) -> Result<PathBuf, String> {
    let mut path = env::temp_dir();
    path.push(format!(
        "omc-team-http-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(
        &path,
        serde_json::to_vec(body).map_err(|e| format!("failed to encode JSON body: {e}"))?,
    )
    .map_err(|e| format!("failed to write temp request body: {e}"))?;
    Ok(path)
}

fn github_get_issue(
    api: &GitHubApi,
    number: u64,
    with_comments: bool,
) -> Result<ExternalIssue, String> {
    let value = github_request(
        api,
        "GET",
        &format!("/repos/{}/issues/{number}", api.repo),
        None,
    )?;
    let mut issue = github_value_to_issue(&api.repo, &value)?;
    if with_comments {
        issue.comments = github_issue_comments(api, number)?;
    }
    Ok(issue)
}

fn github_value_to_issue(repo: &str, value: &Value) -> Result<ExternalIssue, String> {
    let number = value
        .get("number")
        .and_then(Value::as_u64)
        .ok_or("GitHub issue response missing number")?;
    let labels = value
        .get("labels")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|label| label.get("name").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(ExternalIssue {
        id: number.to_string(),
        issue_ref: format!("{repo}#{number}"),
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        body: value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        state: value
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("open")
            .to_string(),
        labels,
        number: Some(number),
        team_name: None,
        comments: Vec::new(),
    })
}

fn github_issue_to_card(repo: &str, issue: &ExternalIssue) -> TaskCard {
    TaskCard {
        meta: TaskMetadata {
            id: issue.issue_ref.clone(),
            title: issue.title.clone(),
            agent_ready: is_ready_labels(&issue.labels),
            risk: "medium".to_string(),
            ownership: section_or_default(
                &issue.body,
                "Ownership",
                "Derive ownership from the GitHub issue before editing.",
            ),
            acceptance: section_or_default(
                &issue.body,
                "Acceptance",
                "Satisfy the GitHub issue acceptance criteria or document gaps.",
            ),
            verification: section_or_default(
                &issue.body,
                "Verification",
                "Run the relevant project checks before handoff.",
            ),
            source: Some("github".to_string()),
            linear_id: None,
            tracker: Some("github".to_string()),
            github_repo: Some(repo.to_string()),
            github_issue_number: issue.number,
        },
        body: issue.body.clone(),
    }
}

fn github_ready_with_api(api: &GitHubApi, limit: usize) -> Result<Vec<ReadyIssue>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let now = unix_timestamp();
    let mut ready = Vec::new();
    let mut page = 1;
    while ready.len() < limit {
        let value = github_request(
            api,
            "GET",
            &format!(
                "/repos/{}/issues?state=open&labels={}&per_page=100&page={page}",
                api.repo,
                percent_encode(AGENT_READY)
            ),
            None,
        )?;
        let Some(items) = value.as_array() else {
            return Err("GitHub ready response was not an array".to_string());
        };
        if items.is_empty() {
            break;
        }
        for item in items {
            if item.get("pull_request").is_some() {
                continue;
            }
            let mut issue = github_value_to_issue(&api.repo, item)?;
            if github_progress_labels(&issue.labels) {
                continue;
            }
            let Some(number) = issue.number else {
                continue;
            };
            issue.comments = github_issue_comments(api, number)?;
            if earliest_active_lease(&issue.comments, now).is_some() {
                continue;
            }
            ready.push(ReadyIssue {
                tracker: TrackerKind::GitHub,
                repo_or_team: api.repo.clone(),
                issue_ref: issue.issue_ref,
                title: issue.title,
                state: issue.state,
                labels: issue.labels,
            });
            if ready.len() >= limit {
                break;
            }
        }
        page += 1;
    }
    Ok(ready)
}

fn github_issue_comments(api: &GitHubApi, number: u64) -> Result<Vec<TrackerComment>, String> {
    let mut comments = Vec::new();
    let mut page = 1;
    loop {
        let value = github_request(
            api,
            "GET",
            &format!(
                "/repos/{}/issues/{number}/comments?per_page=100&page={page}",
                api.repo
            ),
            None,
        )?;
        let Some(items) = value.as_array() else {
            return Ok(comments);
        };
        if items.is_empty() {
            break;
        }
        comments.extend(items.iter().map(|item| {
            TrackerComment {
                id: item
                    .get("id")
                    .map(value_id)
                    .unwrap_or_else(|| "".to_string()),
                body: item
                    .get("body")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                created_at: item
                    .get("created_at")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            }
        }));
        if items.len() < 100 {
            break;
        }
        page += 1;
    }
    Ok(comments)
}

fn github_claim_issue(repo: &str, number: u64, run_id: &str) -> Result<String, String> {
    let api = GitHubApi {
        repo: repo.to_string(),
        token: github_token()?,
    };
    let issue = github_get_issue(&api, number, true)?;
    if !is_ready_labels(&issue.labels) {
        return Err(format!("{} is missing {AGENT_READY}", issue.issue_ref));
    }
    if github_progress_labels(&issue.labels) {
        return Err(format!(
            "{} already has OMC progress labels",
            issue.issue_ref
        ));
    }
    let body = lease_comment(run_id);
    let created = github_request(
        &api,
        "POST",
        &format!("/repos/{}/issues/{number}/comments", api.repo),
        Some(json!({ "body": body })),
    )?;
    let created_id = created
        .get("id")
        .map(value_id)
        .ok_or("GitHub did not return created comment id")?;
    let comments = github_issue_comments(&api, number)?;
    let winner = earliest_active_lease(&comments, unix_timestamp())
        .ok_or("GitHub lease comment was not found after creation")?;
    if winner.id == created_id {
        Ok(created_id)
    } else {
        Err(format!(
            "lost GitHub claim race for {}; winning lease comment is {}",
            issue.issue_ref, winner.id
        ))
    }
}

fn github_mark_started(record: &RunRecord) -> Result<Option<String>, String> {
    let api = GitHubApi {
        repo: record.repo_or_team.clone(),
        token: github_token()?,
    };
    let number = record
        .issue_id
        .parse::<u64>()
        .map_err(|_| format!("invalid GitHub issue number: {}", record.issue_id))?;
    github_add_labels(&api, number, &[GITHUB_CLAIMED, GITHUB_IN_PROGRESS])?;
    github_remove_label(&api, number, AGENT_READY)?;
    let body = format!(
        "{}run_id={} phase=start -->\nOMC team `{}` started for `{}`.\n\nMission: `{}`",
        RUN_PREFIX, record.run_id, record.team_name, record.issue_ref, record.mission_path
    );
    let created = github_request(
        &api,
        "POST",
        &format!("/repos/{}/issues/{number}/comments", api.repo),
        Some(json!({ "body": body })),
    )?;
    Ok(created.get("id").map(value_id))
}

fn github_post_handoff(
    record: &RunRecord,
    summary: &str,
    done: bool,
) -> Result<Option<String>, String> {
    let api = GitHubApi {
        repo: record.repo_or_team.clone(),
        token: github_token()?,
    };
    let number = record
        .issue_id
        .parse::<u64>()
        .map_err(|_| format!("invalid GitHub issue number: {}", record.issue_id))?;
    let body = format!(
        "{}run_id={} phase=handoff -->\n{}",
        RUN_PREFIX,
        record.run_id,
        summary.trim()
    );
    let comment_id = if let Some(id) = &record.handoff_comment_id {
        let updated = github_request(
            &api,
            "PATCH",
            &format!("/repos/{}/issues/comments/{id}", api.repo),
            Some(json!({ "body": body })),
        )?;
        updated.get("id").map(value_id)
    } else {
        let created = github_request(
            &api,
            "POST",
            &format!("/repos/{}/issues/{number}/comments", api.repo),
            Some(json!({ "body": body })),
        )?;
        created.get("id").map(value_id)
    };
    github_remove_label(&api, number, GITHUB_IN_PROGRESS)?;
    if done {
        github_remove_label(&api, number, GITHUB_IN_REVIEW)?;
        github_add_labels(&api, number, &[GITHUB_DONE])?;
        github_request(
            &api,
            "PATCH",
            &format!("/repos/{}/issues/{number}", api.repo),
            Some(json!({ "state": "closed", "state_reason": "completed" })),
        )?;
    } else {
        github_add_labels(&api, number, &[GITHUB_IN_REVIEW])?;
    }
    Ok(comment_id)
}

fn github_label_set(api: &GitHubApi) -> Result<HashSet<String>, String> {
    let mut labels = HashSet::new();
    let mut page = 1;
    loop {
        let value = github_request(
            api,
            "GET",
            &format!("/repos/{}/labels?per_page=100&page={page}", api.repo),
            None,
        )?;
        let Some(items) = value.as_array() else {
            return Ok(labels);
        };
        if items.is_empty() {
            break;
        }
        labels.extend(
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .map(ToString::to_string),
        );
        if items.len() < 100 {
            break;
        }
        page += 1;
    }
    Ok(labels)
}

fn github_create_label(api: &GitHubApi, label: &str) -> Result<(), String> {
    let (color, description) = match label {
        AGENT_READY => ("0E8A16", "Ready for OMC agent team intake"),
        GITHUB_CLAIMED => ("5319E7", "Claimed by an OMC team lease"),
        GITHUB_IN_PROGRESS => ("1D76DB", "OMC team is working"),
        GITHUB_IN_REVIEW => ("FBCA04", "OMC team handoff is ready for review"),
        GITHUB_DONE => ("0E8A16", "Completed by OMC team"),
        _ => ("BFDADC", "OMC team label"),
    };
    github_request(
        api,
        "POST",
        &format!("/repos/{}/labels", api.repo),
        Some(json!({
            "name": label,
            "color": color,
            "description": description
        })),
    )?;
    Ok(())
}

fn github_add_labels(api: &GitHubApi, number: u64, labels: &[&str]) -> Result<(), String> {
    github_request(
        api,
        "POST",
        &format!("/repos/{}/issues/{number}/labels", api.repo),
        Some(json!({ "labels": labels })),
    )?;
    Ok(())
}

fn github_remove_label(api: &GitHubApi, number: u64, label: &str) -> Result<(), String> {
    match github_request(
        api,
        "DELETE",
        &format!(
            "/repos/{}/issues/{number}/labels/{}",
            api.repo,
            percent_encode(label)
        ),
        None,
    ) {
        Ok(_) => Ok(()),
        Err(err) if err.contains("HTTP 404") => Ok(()),
        Err(err) => Err(err),
    }
}

fn github_required_labels() -> &'static [&'static str] {
    &[
        AGENT_READY,
        GITHUB_CLAIMED,
        GITHUB_IN_PROGRESS,
        GITHUB_IN_REVIEW,
        GITHUB_DONE,
    ]
}

fn linear_api() -> Result<LinearApi, String> {
    if let Ok(token) = env::var("LINEAR_API_KEY")
        && !token.trim().is_empty()
    {
        return Ok(LinearApi { token });
    }
    let Some(home) = dirs::home_dir() else {
        return Err("Linear token missing. Set LINEAR_API_KEY.".to_string());
    };
    let path = home.join(".config/linear/config.json");
    let raw = fs::read_to_string(&path).map_err(|_| {
        format!(
            "Linear token missing. Set LINEAR_API_KEY or create {} with api_key.",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("invalid Linear config {}: {e}", path.display()))?;
    let token = value
        .get("api_key")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "Linear token missing. Set LINEAR_API_KEY or create {} with api_key.",
                path.display()
            )
        })?;
    Ok(LinearApi {
        token: token.to_string(),
    })
}

fn linear_graphql(api: &LinearApi, query: &str, variables: Value) -> Result<Value, String> {
    let value = http_request_json(
        "POST",
        "https://api.linear.app/graphql",
        &[
            format!("Authorization: {}", api.token),
            "Content-Type: application/json".to_string(),
            "User-Agent: omc-team".to_string(),
        ],
        Some(json!({ "query": query, "variables": variables })),
    )?;
    if let Some(errors) = value.get("errors") {
        return Err(format!("Linear GraphQL error: {errors}"));
    }
    Ok(value.get("data").cloned().unwrap_or(Value::Null))
}

fn linear_find_team(api: &LinearApi, team: &str) -> Result<Value, String> {
    let data = linear_graphql(
        api,
        r#"
        query Teams {
          teams {
            nodes { id name key }
          }
        }
        "#,
        json!({}),
    )?;
    data["teams"]["nodes"]
        .as_array()
        .and_then(|teams| {
            teams.iter().find(|item| {
                item.get("name").and_then(Value::as_str) == Some(team)
                    || item.get("key").and_then(Value::as_str) == Some(team)
                    || item.get("id").and_then(Value::as_str) == Some(team)
            })
        })
        .cloned()
        .ok_or_else(|| format!("Linear team not found: {team}"))
}

fn linear_team_states(api: &LinearApi, team_id: &str) -> Result<HashSet<String>, String> {
    let data = linear_graphql(
        api,
        r#"
        query TeamStates($id: String!) {
          team(id: $id) {
            states { nodes { id name type } }
          }
        }
        "#,
        json!({ "id": team_id }),
    )?;
    Ok(data["team"]["states"]["nodes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect())
}

fn linear_team_labels(api: &LinearApi, team_id: &str) -> Result<HashSet<String>, String> {
    let data = linear_graphql(
        api,
        r#"
        query TeamLabels($id: String!) {
          team(id: $id) {
            labels { nodes { id name } }
          }
        }
        "#,
        json!({ "id": team_id }),
    )?;
    Ok(data["team"]["labels"]["nodes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect())
}

fn linear_create_label(api: &LinearApi, team_id: &str, label: &str) -> Result<(), String> {
    linear_graphql(
        api,
        r##"
        mutation CreateLabel($name: String!, $teamId: String!) {
          issueLabelCreate(input: { name: $name, color: "#0E8A16", teamId: $teamId }) {
            issueLabel { id name }
          }
        }
        "##,
        json!({ "name": label, "teamId": team_id }),
    )?;
    Ok(())
}

fn linear_find_issue(api: &LinearApi, issue_ref: &str) -> Result<ExternalIssue, String> {
    let data = linear_graphql(
        api,
        r#"
        query Issue($id: String!) {
          issue(id: $id) {
            id identifier title description
            state { id name }
            team { id name key }
            labels { nodes { id name } }
            comments(first: 100) { nodes { id body createdAt } }
          }
        }
        "#,
        json!({ "id": issue_ref }),
    )?;
    if !data["issue"].is_null() {
        return linear_value_to_issue(&data["issue"]);
    }
    let Some((team_key, _)) = issue_ref.split_once('-') else {
        return Err(format!("Linear issue not found: {issue_ref}"));
    };
    linear_find_issue_by_identifier(api, team_key, issue_ref)
}

fn linear_find_issue_by_identifier(
    api: &LinearApi,
    team_key: &str,
    issue_ref: &str,
) -> Result<ExternalIssue, String> {
    let mut after = Value::Null;
    loop {
        let data = linear_graphql(
            api,
            r#"
            query Issues($teamKey: String!, $after: String) {
              issues(filter: { team: { key: { eq: $teamKey } } }, first: 100, after: $after) {
                pageInfo { hasNextPage endCursor }
                nodes {
                  id identifier title description
                  state { id name }
                  team { id name key }
                  labels { nodes { id name } }
                  comments(first: 100) { nodes { id body createdAt } }
                }
              }
            }
            "#,
            json!({ "teamKey": team_key, "after": after }),
        )?;
        let nodes = data["issues"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for node in nodes {
            if node.get("identifier").and_then(Value::as_str) == Some(issue_ref) {
                return linear_value_to_issue(&node);
            }
        }
        if !data["issues"]["pageInfo"]["hasNextPage"]
            .as_bool()
            .unwrap_or(false)
        {
            break;
        }
        after = data["issues"]["pageInfo"]["endCursor"].clone();
    }
    Err(format!("Linear issue not found: {issue_ref}"))
}

fn linear_team_issues(
    api: &LinearApi,
    team_id: &str,
    wanted: usize,
) -> Result<Vec<ExternalIssue>, String> {
    let mut after = Value::Null;
    let mut out = Vec::new();
    loop {
        let data = linear_graphql(
            api,
            r#"
            query TeamIssues($teamId: String!, $after: String) {
              issues(filter: { team: { id: { eq: $teamId } } }, first: 100, after: $after) {
                pageInfo { hasNextPage endCursor }
                nodes {
                  id identifier title description
                  state { id name }
                  team { id name key }
                  labels { nodes { id name } }
                  comments(first: 100) { nodes { id body createdAt } }
                }
              }
            }
            "#,
            json!({ "teamId": team_id, "after": after }),
        )?;
        for node in data["issues"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            out.push(linear_value_to_issue(&node)?);
            if out.len() >= wanted {
                return Ok(out);
            }
        }
        if !data["issues"]["pageInfo"]["hasNextPage"]
            .as_bool()
            .unwrap_or(false)
        {
            break;
        }
        after = data["issues"]["pageInfo"]["endCursor"].clone();
    }
    Ok(out)
}

fn linear_value_to_issue(value: &Value) -> Result<ExternalIssue, String> {
    let labels = value["labels"]["nodes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|label| label.get("name").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let comments = value["comments"]["nodes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|item| TrackerComment {
            id: item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            body: item
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            created_at: item
                .get("createdAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
        .collect();
    Ok(ExternalIssue {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .ok_or("Linear issue response missing id")?
            .to_string(),
        issue_ref: value
            .get("identifier")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        body: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        state: value["state"]["name"].as_str().unwrap_or("").to_string(),
        labels,
        number: None,
        team_name: value["team"]["name"].as_str().map(ToString::to_string),
        comments,
    })
}

fn linear_issue_to_card(issue: &ExternalIssue) -> TaskCard {
    TaskCard {
        meta: TaskMetadata {
            id: issue.issue_ref.clone(),
            title: issue.title.clone(),
            agent_ready: is_ready_labels(&issue.labels),
            risk: "medium".to_string(),
            ownership: section_or_default(
                &issue.body,
                "Ownership",
                "Derive ownership from the Linear issue before editing.",
            ),
            acceptance: section_or_default(
                &issue.body,
                "Acceptance",
                "Satisfy the Linear issue acceptance criteria or document gaps.",
            ),
            verification: section_or_default(
                &issue.body,
                "Verification",
                "Run the relevant project checks before handoff.",
            ),
            source: Some("linear".to_string()),
            linear_id: Some(issue.issue_ref.clone()),
            tracker: Some("linear".to_string()),
            github_repo: None,
            github_issue_number: None,
        },
        body: issue.body.clone(),
    }
}

fn linear_claim_issue(issue_id: &str, run_id: &str) -> Result<String, String> {
    let api = linear_api()?;
    let before = linear_find_issue(&api, issue_id)?;
    if !is_ready_labels(&before.labels) {
        return Err(format!("{} is missing {AGENT_READY}", before.issue_ref));
    }
    if linear_progress_state(&before.state) {
        return Err(format!(
            "{} is already in progress state `{}`",
            before.issue_ref, before.state
        ));
    }
    let body = lease_comment(run_id);
    let created = linear_graphql(
        &api,
        r#"
        mutation CommentCreate($issueId: String!, $body: String!) {
          commentCreate(input: { issueId: $issueId, body: $body }) {
            comment { id body createdAt }
          }
        }
        "#,
        json!({ "issueId": issue_id, "body": body }),
    )?;
    let created_id = created["commentCreate"]["comment"]["id"]
        .as_str()
        .ok_or("Linear did not return created comment id")?
        .to_string();
    let issue = linear_find_issue(&api, issue_id)?;
    let winner = earliest_active_lease(&issue.comments, unix_timestamp())
        .ok_or("Linear lease comment was not found after creation")?;
    if winner.id == created_id {
        Ok(created_id)
    } else {
        Err(format!(
            "lost Linear claim race for {}; winning lease comment is {}",
            issue.issue_ref, winner.id
        ))
    }
}

fn linear_mark_started(record: &RunRecord) -> Result<Option<String>, String> {
    let api = linear_api()?;
    let body = format!(
        "{}run_id={} phase=start -->\nOMC team `{}` started for `{}`.\n\nMission: `{}`",
        RUN_PREFIX, record.run_id, record.team_name, record.issue_ref, record.mission_path
    );
    let created = linear_graphql(
        &api,
        r#"
        mutation CommentCreate($issueId: String!, $body: String!) {
          commentCreate(input: { issueId: $issueId, body: $body }) {
            comment { id }
          }
        }
        "#,
        json!({ "issueId": record.issue_id, "body": body }),
    )?;
    let comment_id = created["commentCreate"]["comment"]["id"]
        .as_str()
        .map(ToString::to_string);
    linear_update_state_by_name(&api, &record.issue_id, "In Progress")?;
    Ok(comment_id)
}

fn linear_post_handoff(
    record: &RunRecord,
    summary: &str,
    done: bool,
) -> Result<Option<String>, String> {
    let api = linear_api()?;
    let body = format!(
        "{}run_id={} phase=handoff -->\n{}",
        RUN_PREFIX,
        record.run_id,
        summary.trim()
    );
    let comment_id = if let Some(id) = &record.handoff_comment_id {
        let updated = linear_graphql(
            &api,
            r#"
            mutation CommentUpdate($id: String!, $body: String!) {
              commentUpdate(id: $id, input: { body: $body }) {
                comment { id }
              }
            }
            "#,
            json!({ "id": id, "body": body }),
        )?;
        updated["commentUpdate"]["comment"]["id"]
            .as_str()
            .map(ToString::to_string)
    } else {
        let created = linear_graphql(
            &api,
            r#"
            mutation CommentCreate($issueId: String!, $body: String!) {
              commentCreate(input: { issueId: $issueId, body: $body }) {
                comment { id }
              }
            }
            "#,
            json!({ "issueId": record.issue_id, "body": body }),
        )?;
        created["commentCreate"]["comment"]["id"]
            .as_str()
            .map(ToString::to_string)
    };
    linear_update_state_by_name(
        &api,
        &record.issue_id,
        if done { "Done" } else { "In Review" },
    )?;
    Ok(comment_id)
}

fn linear_update_state_by_name(
    api: &LinearApi,
    issue_id: &str,
    state_name: &str,
) -> Result<(), String> {
    let issue = linear_find_issue(api, issue_id)?;
    let data = linear_graphql(
        api,
        r#"
        query IssueTeam($id: String!) {
          issue(id: $id) { team { id } }
        }
        "#,
        json!({ "id": issue.id }),
    )?;
    let Some(team_id) = data["issue"]["team"]["id"].as_str() else {
        return Ok(());
    };
    let states_data = linear_graphql(
        api,
        r#"
        query TeamStates($id: String!) {
          team(id: $id) {
            states { nodes { id name } }
          }
        }
        "#,
        json!({ "id": team_id }),
    )?;
    let Some(state_id) = states_data["team"]["states"]["nodes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .find(|state| state.get("name").and_then(Value::as_str) == Some(state_name))
        .and_then(|state| state.get("id").and_then(Value::as_str))
    else {
        return Ok(());
    };
    linear_graphql(
        api,
        r#"
        mutation IssueUpdate($id: String!, $stateId: String!) {
          issueUpdate(id: $id, input: { stateId: $stateId }) {
            issue { id }
          }
        }
        "#,
        json!({ "id": issue.id, "stateId": state_id }),
    )?;
    Ok(())
}

fn lease_comment(run_id: &str) -> String {
    format!(
        "{LEASE_PREFIX}run_id={run_id} expires_at={} {COMMENT_END}\nOMC team lease for `{run_id}`.",
        unix_timestamp() + LEASE_TTL_SECONDS
    )
}

fn earliest_active_lease(comments: &[TrackerComment], now: u64) -> Option<TrackerComment> {
    let mut leases = comments
        .iter()
        .filter_map(|comment| parse_lease(&comment.body).map(|lease| (comment, lease)))
        .filter(|(_, lease)| lease.expires_at > now)
        .collect::<Vec<_>>();
    leases.sort_by(|(left_comment, left_lease), (right_comment, right_lease)| {
        left_comment
            .created_at
            .cmp(&right_comment.created_at)
            .then_with(|| left_comment.id.cmp(&right_comment.id))
            .then_with(|| left_lease.run_id.cmp(&right_lease.run_id))
    });
    leases.first().map(|(comment, _)| (*comment).clone())
}

fn parse_lease(body: &str) -> Option<Lease> {
    let start = body.find(LEASE_PREFIX)? + LEASE_PREFIX.len();
    let end = body[start..].find(COMMENT_END)? + start;
    let attrs = &body[start..end];
    let mut run_id = None;
    let mut expires_at = None;
    for part in attrs.split_whitespace() {
        if let Some(value) = part.strip_prefix("run_id=") {
            run_id = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("expires_at=") {
            expires_at = value.parse::<u64>().ok();
        }
    }
    Some(Lease {
        run_id: run_id?,
        expires_at: expires_at?,
    })
}

fn is_ready_labels(labels: &[String]) -> bool {
    labels.iter().any(|label| label == AGENT_READY)
}

fn github_progress_labels(labels: &[String]) -> bool {
    labels.iter().any(|label| {
        matches!(
            label.as_str(),
            GITHUB_CLAIMED | GITHUB_IN_PROGRESS | GITHUB_IN_REVIEW | GITHUB_DONE
        )
    })
}

fn linear_progress_state(state: &str) -> bool {
    matches!(
        state.to_ascii_lowercase().as_str(),
        "in progress" | "in review" | "done" | "canceled" | "cancelled" | "duplicate"
    )
}

fn section_or_default(body: &str, heading: &str, default: &str) -> Vec<String> {
    parse_markdown_section(body, heading).unwrap_or_else(|| vec![default.to_string()])
}

fn parse_markdown_section(body: &str, heading: &str) -> Option<Vec<String>> {
    let mut in_section = false;
    let mut lines = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let normalized = trimmed.trim_start_matches('#').trim();
        if trimmed.starts_with('#') {
            if normalized.eq_ignore_ascii_case(heading) {
                in_section = true;
                continue;
            }
            if in_section {
                break;
            }
        }
        if in_section && !trimmed.is_empty() {
            lines.push(trimmed.trim_start_matches("- ").to_string());
        }
    }
    if lines.is_empty() { None } else { Some(lines) }
}

fn value_id(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(|id| id.to_string()))
        .unwrap_or_default()
}

fn percent_encode(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn is_digits(raw: &str) -> bool {
    !raw.is_empty() && raw.chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_issue_refs() {
        assert_eq!(
            parse_github_issue_ref("#123", Some("owner/repo")).unwrap(),
            ParsedGitHubIssue {
                repo: "owner/repo".to_string(),
                number: 123
            }
        );
        assert_eq!(
            parse_github_issue_ref("octo/hello#9", None).unwrap(),
            ParsedGitHubIssue {
                repo: "octo/hello".to_string(),
                number: 9
            }
        );
        assert_eq!(
            parse_github_issue_ref("https://github.com/octo/hello/issues/42", None).unwrap(),
            ParsedGitHubIssue {
                repo: "octo/hello".to_string(),
                number: 42
            }
        );
    }

    #[test]
    fn parses_github_remotes() {
        assert_eq!(
            parse_github_remote("https://github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
        assert_eq!(
            parse_github_remote("git@github.com:owner/repo.git"),
            Some("owner/repo".to_string())
        );
        assert_eq!(
            parse_github_remote("ssh://git@github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }

    #[test]
    fn ready_filter_requires_agent_ready_and_excludes_progress() {
        assert!(is_ready_labels(&["agent-ready".to_string()]));
        assert!(!is_ready_labels(&["bug".to_string()]));
        assert!(github_progress_labels(&["omc/in-progress".to_string()]));
        assert!(!github_progress_labels(&["agent-ready".to_string()]));
    }

    #[test]
    fn earliest_lease_wins() {
        let comments = vec![
            TrackerComment {
                id: "2".to_string(),
                body: format!("{LEASE_PREFIX}run_id=late expires_at=9999999999 {COMMENT_END}"),
                created_at: "2026-05-07T00:00:02Z".to_string(),
            },
            TrackerComment {
                id: "1".to_string(),
                body: format!("{LEASE_PREFIX}run_id=early expires_at=9999999999 {COMMENT_END}"),
                created_at: "2026-05-07T00:00:01Z".to_string(),
            },
        ];
        let winner = earliest_active_lease(&comments, 1).unwrap();
        assert_eq!(winner.id, "1");
    }

    #[test]
    fn expired_lease_is_ignored() {
        let comments = vec![TrackerComment {
            id: "1".to_string(),
            body: format!("{LEASE_PREFIX}run_id=old expires_at=10 {COMMENT_END}"),
            created_at: "2026-05-07T00:00:01Z".to_string(),
        }];
        assert!(earliest_active_lease(&comments, 11).is_none());
    }
}
