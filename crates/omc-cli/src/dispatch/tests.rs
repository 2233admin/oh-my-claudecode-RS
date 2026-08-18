use super::templates::parse_frontmatter_description;
use super::*;
use omc_host::HostKind;

#[test]
fn test_substitute_arguments_dollar() {
    let template = "Run with:\n```text\n$ARGUMENTS\n```\n";
    let result = substitute_arguments(template, "hello world");
    assert_eq!(result, "Run with:\n```text\nhello world\n```\n");
}

#[test]
fn test_substitute_arguments_braces() {
    let template = "Task: {{ARGUMENTS}}";
    let result = substitute_arguments(template, "hello world");
    assert_eq!(result, "Task: hello world");
}

#[test]
fn test_substitute_arguments_both_formats() {
    let template = "$ARGUMENTS and {{ARGUMENTS}}";
    let result = substitute_arguments(template, "test");
    assert_eq!(result, "test and test");
}

#[test]
fn test_substitute_no_placeholder() {
    let template = "No placeholder here";
    let result = substitute_arguments(template, "args");
    assert_eq!(result, "No placeholder here");
}

#[test]
fn test_substitute_empty_arguments() {
    let template = "Args: $ARGUMENTS";
    let result = substitute_arguments(template, "");
    assert_eq!(result, "Args: ");
}

#[test]
fn test_parse_frontmatter_description() {
    let content = r#"---
description: "A test skill"
name: test
---

# Content"#;
    let desc = parse_frontmatter_description(content);
    assert_eq!(desc, Some("A test skill".to_string()));
}

#[test]
fn test_parse_frontmatter_no_description() {
    let content = r#"---
name: test
---

# Content"#;
    let desc = parse_frontmatter_description(content);
    assert_eq!(desc, None);
}

#[test]
fn test_parse_frontmatter_empty() {
    let content = "no frontmatter here";
    let desc = parse_frontmatter_description(content);
    assert_eq!(desc, None);
}

#[test]
fn test_skill_args_joined() {
    let args = SkillArgs {
        args: vec!["hello".into(), "world".into()],
    };
    assert_eq!(args.joined(), "hello world");
}

#[test]
fn test_skill_args_empty() {
    let args = SkillArgs { args: vec![] };
    assert_eq!(args.joined(), "");
}

#[test]
fn test_skill_names_unique() {
    // Verify all commands map to distinct skill names (except aliases)
    let commands = vec![
        "omc-setup",
        "configure-notifications",
        "hud",
        "skill",
        "skillify",
        "trace",
        "verify",
        "visual-verdict",
        "wiki",
        "learner",
        "remember",
        "ask",
        "autoresearch",
        "ccg",
        "cancel",
        "debug",
        "deep-dive",
        "deepinit",
        "external-context",
        "project-session-manager",
        "psm",
        "release",
        "self-improve",
        "omc-teams",
        "plan",
        "deep-interview",
    ];
    let mut sorted = commands.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        commands.len(),
        sorted.len(),
        "duplicate skill names detected"
    );
}

#[test]
fn doctor_reports_selected_ready_host() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
    std::fs::write(tmp.path().join(".codex/config.toml"), "").unwrap();

    let reports = collect_doctor_reports(tmp.path(), Some("codex")).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].host, HostKind::Codex);
    assert!(reports[0].ready);
}

#[test]
fn doctor_rejects_unknown_host() {
    let tmp = tempfile::tempdir().unwrap();
    let error = collect_doctor_reports(tmp.path(), Some("sentinel")).unwrap_err();
    assert!(error.to_string().contains("unknown host"));
}

#[test]
fn status_has_a_stable_schema_and_complete_catalog_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let status = super::status::build_status(tmp.path());
    assert_eq!(status.schema_version, "omc.status.v1");
    assert_eq!(status.capabilities.total, 16);
    assert_eq!(status.capabilities.mcp_tools, 32);
    assert!(status.catalog.ok);
    assert!(status.dependencies.ok);
}

#[test]
fn status_projects_active_unknown_profile_with_provenance() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("future.json");
    std::fs::write(
        &profile,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"omc.profile.v1", "id":"future-agent",
            "runtime":{"id":"future-runtime","command":"future-agent"},
            "provider":{"id":"private-provider"},
            "model":{"id":"private-model","capabilities":["tool-calling"]},
            "protocol":{"kind":"mcp-stdio"},
            "permissions":{"omc":["read","network"],"runtime":["read"]},
            "dependencies":[{"id":"future-agent","kind":"required","commands":["future-agent"]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let status = super::status::build_status_with_profile(tmp.path(), Some(&profile));
    let data = status.runtime_profile.data.unwrap();
    assert_eq!(data["activeProfile"], "future-agent");
    assert_eq!(data["effectivePermissions"], serde_json::json!(["read"]));
    assert_eq!(data["provenance"]["source"], "explicit");
}
