# Test Coverage Analysis — omc-RS

**Date:** 2026-05-11
**Total tests:** 392 (per CLAUDE.md)
**Crates:** 18

---

## Executive Summary

| Category | Crates | Assessment |
|----------|--------|------------|
| Excellent (80%+ files tested) | omc-team, omc-shared, omc-hud | Well covered |
| Good (50-79%) | omc-host, omc-hooks, omc-skills, omc-mcp, omc-interop, omc-context | Adequate |
| Poor (<50%) | **omc-git-provider, omc-wiki, omc-autoresearch, omc-xcmd, omc-cli, omc-notifications** | Needs work |
| N/A | omc-macros (proc-macro, tested via macro_tests) | — |

**Critical gap:** `omc-git-provider` — 66 public functions, ~5% test coverage.
**Integration tests:** Only 1 file (`omc-skills/tests/integration.rs`), no cross-crate integration tests.
**Property-based / fuzz tests:** None.

---

## Crate-by-Crate Gaps

---

### omc-git-provider (~5% coverage — 61 untested functions)

> **Why it matters:** Critical multi-provider git abstraction (GitHub, GitLab, Bitbucket, Azure, Gitea/Forgejo). Any bug in URL detection or CLI invocation affects all providers.

#### Untested public functions by file

**types.rs (0% coverage)**

```rust
// GAP: types::ProviderName — Display impl untested
#[test]
fn test_provider_name_display() {
    assert_eq!(ProviderName::GitHub.to_string(), "github");
    assert_eq!(ProviderName::Unknown.to_string(), "unknown");
    // All 7 variants
}

// GAP: types::PRTerminology — Display impl untested
#[test]
fn test_pr_terminology_display() {
    assert_eq!(PRTerminology::PR.to_string(), "PR");
    assert_eq!(PRTerminology::MR.to_string(), "MR");
}

// GAP: types::ProviderError — all variants untested
#[test]
fn test_provider_error_display() {
    assert_eq!(ProviderError::InvalidInput("bad".into()).to_string(), "invalid input: bad");
    assert_eq!(ProviderError::AuthFailed("no token".into()).to_string(), "authentication failed: no token");
    assert_eq!(ProviderError::NotFound.to_string(), "not found");
    assert_eq!(ProviderError::ApiError("net".into()).to_string(), "API request failed: net");
    assert_eq!(ProviderError::CliNotFound("gh".into()).to_string(), "CLI not found: gh");
}

// GAP: types::PRInfo, IssueInfo — construction untested
#[test]
fn test_pr_info_construction() {
    let info = PRInfo { number: 42, title: "Fix bug".into(), state: "open".into(), url: "https://...".into() };
    assert_eq!(info.number, 42);
}
```

**lib.rs — partial coverage, missing error/edge paths**

```rust
// GAP: lib::get_provider — not tested at all
#[test]
fn test_get_provider_all_variants() {
    assert!(get_provider(ProviderName::GitHub).is_some());
    assert!(get_provider(ProviderName::Unknown).is_none());
}

// GAP: lib::detect_provider — empty/invalid input not tested
#[test]
fn test_detect_provider_empty() { assert_eq!(detect_provider(""), ProviderName::Unknown); }
#[test]
fn test_detect_provider_invalid() { assert_eq!(detect_provider("not-a-url"), ProviderName::Unknown); }

// GAP: lib::parse_remote_url — missing edge cases
#[test]
fn test_parse_remote_url_empty() { assert_eq!(parse_remote_url(""), None); }
#[test]
fn test_parse_remote_url_whitespace() { assert_eq!(parse_remote_url("   "), None); }
#[test]
fn test_parse_remote_url_no_host() { assert_eq!(parse_remote_url("https://github.com"), None); }
#[test]
fn test_parse_remote_url_azure_ssh() {
    let info = parse_remote_url("git@ssh.dev.azure.com:v3/org/project/repo").unwrap();
    assert_eq!(info.provider, ProviderName::AzureDevOps);
}
#[test]
fn test_parse_remote_url_azure_legacy_https() {
    let info = parse_remote_url("https://myorg.visualstudio.com/p/_git/r").unwrap();
    assert_eq!(info.provider, ProviderName::AzureDevOps);
}
#[test]
fn test_parse_remote_url_strips_git_suffix() {
    assert_eq!(parse_remote_url("https://github.com/o/r.git").unwrap().repo, "r");
}
#[test]
fn test_parse_remote_url_nested_gitlab() {
    let info = parse_remote_url("https://gitlab.com/g/s/ss/r").unwrap();
    assert_eq!(info.owner, "g/s/ss");
    assert_eq!(info.repo, "r");
}
#[test]
fn test_parse_remote_url_selfhosted_github_enterprise() {
    assert_eq!(parse_remote_url("https://github.myco.com/o/r").unwrap().provider, ProviderName::GitHub);
}
#[test]
fn test_parse_remote_url_selfhosted_gitlab() {
    assert_eq!(parse_remote_url("https://gitlab.internal.co/o/r").unwrap().provider, ProviderName::GitLab);
}
```

**github.rs (0% coverage)**

```rust
// GAP: github — all 10 methods untested
#[test]
fn test_github_provider_identity() {
    let p = GitHubProvider::new();
    assert_eq!(p.name(), ProviderName::GitHub);
    assert_eq!(p.display_name(), "GitHub");
    assert_eq!(p.pr_terminology(), PRTerminology::PR);
    assert_eq!(p.pr_refspec(), Some("pull/{number}/head:{branch}"));
    assert_eq!(p.required_cli(), Some("gh"));
}

#[test]
fn test_github_detect_from_remote() {
    let p = GitHubProvider::new();
    assert!(p.detect_from_remote("https://github.com/o/r"));
    assert!(p.detect_from_remote("git@github.com:o/r"));
    assert!(!p.detect_from_remote("https://gitlab.com/o/r"));
}

#[tokio::test]
async fn test_github_view_pr_invalid_number() {
    let p = GitHubProvider::new();
    assert!(matches!(p.view_pr(0, Some("o"), Some("r")).await, Err(ProviderError::InvalidInput(_))));
    assert!(matches!(p.view_pr(-1, Some("o"), Some("r")).await, Err(ProviderError::InvalidInput(_))));
}

#[tokio::test]
async fn test_github_view_issue_invalid_number() {
    let p = GitHubProvider::new();
    assert!(matches!(p.view_issue(0, Some("o"), Some("r")).await, Err(ProviderError::InvalidInput(_))));
}
```

**gitlab.rs (0% coverage)**

```rust
// GAP: gitlab — all 13 methods untested
#[test]
fn test_gitlab_provider_identity() {
    let p = GitLabProvider::new();
    assert_eq!(p.name(), ProviderName::GitLab);
    assert_eq!(p.display_name(), "GitLab");
    assert_eq!(p.pr_terminology(), PRTerminology::MR);
    assert_eq!(p.required_cli(), Some("glab"));
}

#[test]
fn test_gitlab_extract_host_from_url() {
    assert_eq!(extract_host_from_url("git@github.com:o/r"), "github.com");
    assert_eq!(extract_host_from_url("https://gitlab.com/o/r"), "gitlab.com");
    assert_eq!(extract_host_from_url(""), "");
}

#[test]
fn test_gitlab_host_label_matches() {
    assert!(host_label_matches("gitlab.example.com", "gitlab"));
    assert!(host_label_matches("my-gitlab-server", "gitlab"));
    assert!(host_label_matches("gitlab-ci.co", "gitlab"));
    assert!(!host_label_matches("github.com", "gitlab"));
}

#[tokio::test]
async fn test_gitlab_view_pr_invalid_number() {
    let p = GitLabProvider::new();
    assert!(matches!(p.view_pr(0, Some("o"), Some("r")).await, Err(ProviderError::InvalidInput(_))));
}

#[tokio::test]
async fn test_gitlab_detect_from_api_unavailable() {
    let p = GitLabProvider::new();
    assert!(!p.detect_from_api("https://nonexistent.example.com").await);
}
```

**bitbucket.rs (0% coverage)**

```rust
// GAP: bitbucket — all 10 methods + base64_encode untested
#[test]
fn test_bitbucket_provider_identity() {
    let p = BitbucketProvider::new();
    assert_eq!(p.name(), ProviderName::Bitbucket);
    assert_eq!(p.required_cli(), None); // Uses REST API, no CLI
}

#[test]
fn test_base64_encode_edge_cases() {
    assert_eq!(base64_encode(""), "");
    assert_eq!(base64_encode("a"), "YQ==");
    assert_eq!(base64_encode("ab"), "YWI=");
    assert_eq!(base64_encode("abc"), "YWJj");     // No padding
    assert_eq!(base64_encode("user:pass"), "dXNlcjpwYXNz");
}

#[tokio::test]
async fn test_bitbucket_view_pr_invalid_number() {
    let p = BitbucketProvider::new();
    assert!(matches!(p.view_pr(0, Some("o"), Some("r")).await, Err(ProviderError::InvalidInput(_))));
}
```

**azure_devops.rs (0% coverage)**

```rust
// GAP: azure_devops — all 10 methods + strip_ref_prefix untested
#[test]
fn test_azure_devops_provider_identity() {
    let p = AzureDevOpsProvider::new();
    assert_eq!(p.name(), ProviderName::AzureDevOps);
    assert_eq!(p.required_cli(), Some("az"));
}

#[test]
fn test_strip_ref_prefix() {
    assert_eq!(strip_ref_prefix("refs/heads/main"), "main");
    assert_eq!(strip_ref_prefix("refs/heads/feature/test"), "feature/test");
    assert_eq!(strip_ref_prefix("refs/heads/"), "");
    assert_eq!(strip_ref_prefix("main"), "main");       // No prefix
    assert_eq!(strip_ref_prefix(""), "");                 // Empty
}

#[tokio::test]
async fn test_azure_devops_view_pr_invalid_number() {
    let p = AzureDevOpsProvider::new();
    assert!(matches!(p.view_pr(0, None, None).await, Err(ProviderError::InvalidInput(_))));
}
```

**gitea.rs (0% coverage)**

```rust
// GAP: gitea — all 11 methods untested
#[test]
fn test_gitea_provider_identity() {
    let p = GitteaProvider::new();
    assert_eq!(p.name(), ProviderName::Gitea);
    let f = GitteaProvider::forgejo();
    assert_eq!(f.name(), ProviderName::Forgejo);
}

#[tokio::test]
async fn test_gitea_view_pr_rest_auth_failed_no_url() {
    let p = GitteaProvider::new();
    assert!(matches!(
        p.view_pr_rest("", None, "o", "r").await,
        Err(ProviderError::AuthFailed(_))
    ));
}

#[tokio::test]
async fn test_gitea_detect_from_api() {
    let p = GitteaProvider::new();
    assert!(!p.detect_from_api("https://nonexistent.example.com").await);
}
```

---

### omc-wiki (~6% coverage — 16 untested functions, 7 untested helpers)

> **Why it matters:** Core wiki knowledge layer. All YAML frontmatter parsing/serialization is untested, as are the main operations (ingest, query, lint).

```rust
// === storage.rs ===

// GAP: parse_frontmatter — core YAML parsing, completely untested
#[test]
fn test_parse_frontmatter_valid() {
    let input = "---\nname: test\n---\nContent";
    assert_eq!(parse_frontmatter(input), Some(("name: test\n".into(), "Content".into())));
}
#[test]
fn test_parse_frontmatter_missing_delimiter() { assert!(parse_frontmatter("no delimiter").is_none()); }
#[test]
fn test_parse_frontmatter_empty_content() { assert!(parse_frontmatter("---\n---\n").is_some()); }
#[test]
fn test_parse_frontmatter_no_frontmatter() { assert!(parse_frontmatter("plain text").is_none()); }

// GAP: serialize_page — YAML serialization untested
#[tokio::test]
async fn test_serialize_page_basic() {
    let page = crate::WikiPage { name: "Test".into(), slug: "test".into(), content: "Hello".into(),
        tags: vec![], links: vec![], category: None, confidence: 1.0, source: None };
    let out = serialize_page(&page).await.unwrap();
    assert!(out.contains("name: test"));
    assert!(out.contains("Hello"));
}
#[tokio::test]
async fn test_serialize_page_yaml_escaping() {
    // Newlines, quotes, colons in content should be YAML-safe
}

// GAP: write_page — error paths not tested
#[tokio::test]
async fn test_write_page_reserved_filename() {
    assert!(write_page(&dir, "index.md", "body").await.is_err());
    assert!(write_page(&dir, "log.md", "body").await.is_err());
}
#[tokio::test]
async fn test_write_page_path_traversal() {
    assert!(write_page(&dir, "../etc/passwd", "body").await.is_err());
}

// GAP: read_page — error paths not tested
#[tokio::test]
async fn test_read_page_not_found() { assert_eq!(read_page(&dir, "nonexistent").await?, None); }
#[tokio::test]
async fn test_read_page_corrupt_frontmatter() { /* returns Ok(None) per impl */ }

// GAP: delete_page — not tested
#[tokio::test]
async fn test_delete_page_not_found() { assert_eq!(delete_page(&dir, "nonexistent").await?, false); }
#[tokio::test]
async fn test_delete_page_success() { /* delete existing page */ }

// GAP: list_pages — edge cases not tested
#[tokio::test]
async fn test_list_pages_empty() { assert!(list_pages(&dir).await?.is_empty()); }
#[tokio::test]
async fn test_list_pages_excludes_reserved() { /* index.md, log.md filtered */ }

// GAP: append_log — untested
#[tokio::test]
async fn test_append_log_new_file() { /* creates log.md */ }
#[tokio::test]
async fn test_append_log_existing() { /* appends to existing */ }

// GAP: read_index / read_log — missing file not tested
#[tokio::test]
async fn test_read_index_not_found() { assert_eq!(read_index(&dir).await?, None); }
#[tokio::test]
async fn test_read_log_not_found() { assert_eq!(read_log(&dir).await?, None); }

// GAP: read_all_pages — untested
#[tokio::test]
async fn test_read_all_pages_multiple() { /* aggregation */ }

// GAP: ensure_wiki_dir — untested
#[test]
fn test_ensure_wiki_dir_already_exists() { /* no-op is ok */ }

// === wiki.rs ===

// GAP: ingest_knowledge — main ingest op, untested
#[tokio::test]
async fn test_ingest_knowledge_new_page() {
    // Creates page with frontmatter from text
}
#[tokio::test]
async fn test_ingest_knowledge_merge_existing() { /* dedup tags, append content */ }
#[test]
fn test_ingest_knowledge_confidence_ranking() { /* high confidence wins */ }
#[test]
fn test_ingest_knowledge_extracts_links() { /* [[wiki-link]] -> links array */ }

// GAP: query_wiki — main query op, untested
#[test]
fn test_query_wiki_basic() { /* text match in content */ }
#[test]
fn test_query_wiki_tag_filter() { /* OR match on tags */ }
#[test]
fn test_query_wiki_category_filter() { /* exact match */ }
#[test]
fn test_query_wiki_limit() { /* respects limit */ }
#[test]
fn test_query_wiki_scoring() { /* tag=3, title=5, content=1 */ }
#[test]
fn test_query_wiki_no_matches() { /* empty vec */ }
#[test]
fn test_query_wiki_empty_wiki() { /* empty vec */ }

// GAP: lint_wiki — main lint op, untested
#[test]
fn test_lint_wiki_orphan_detection() { /* pages with no incoming links */ }
#[test]
fn test_lint_wiki_stale_detection() { /* >30 days old */ }
#[test]
fn test_lint_wiki_broken_ref() { /* links to nonexistent pages */ }
#[test]
fn test_lint_wiki_low_confidence() { /* <0.5 confidence */ }
#[test]
fn test_lint_wiki_oversized() { /* >10KB */ }
#[test]
fn test_lint_wiki_contradictions() { /* confidence conflicts */ }
#[test]
fn test_lint_wiki_empty() { /* empty report */ }
#[test]
fn test_lint_wiki_stats() { /* correct counts per issue type */ }

// GAP: tokenize — helper, untested
#[test]
fn test_tokenize_latin() { assert_eq!(tokenize("hello world"), vec!["hello", "world"]); }
#[test]
fn test_tokenize_cjk() { /* chars + bigrams */ }
#[test]
fn test_tokenize_mixed() { /* latin + cjk */ }
#[test]
fn test_tokenize_empty() { assert!(tokenize("").is_empty()); }
#[test]
fn test_tokenize_special_chars() { /* punctuation is delimiter */ }

// GAP: extract_wiki_links — helper, untested
#[test]
fn test_extract_wiki_links_basic() { assert_eq!(extract_wiki_links("see [[Page-Name]]"), vec!["page-name"]); }
#[test]
fn test_extract_wiki_links_multiple() { /* all links */ }
#[test]
fn test_extract_wiki_links_whitespace() { /* trimmed */ }
#[test]
fn test_extract_wiki_links_unclosed() { /* [[ without ]] ignored */ }

// GAP: detect_structural_contradictions — helper, untested
#[test]
fn test_detect_confidence_conflict() { /* high/low on related */ }
#[test]
fn test_detect_tag_category_conflict() { /* same tag diff categories */ }
#[test]
fn test_detect_no_contradictions() { /* consistent pages */ }
```

---

### omc-autoresearch (~15% coverage — 10 untested functions)

> **Why it matters:** Mission/orchestrator runtime for autonomous research. Several error paths and decision logic are untested.

```rust
// === prd.rs ===

// GAP: validate_setup_handoff — input validation, missing edge cases
#[test]
fn test_validate_setup_handoff_empty_mission_text() {
    let h = make_handoff(mission_text: "   ");
    assert!(validate_setup_handoff(&h).is_err());
}
#[test]
fn test_validate_setup_handoff_empty_evaluator() {
    let h = make_handoff(evaluator_command: "   ");
    assert!(validate_setup_handoff(&h).is_err());
}
#[test]
fn test_validate_setup_handoff_confidence_boundaries() {
    // 0.0 and 1.0 should pass, 1.5 should fail
    assert!(validate_setup_handoff(&make_handoff(confidence: 1.5)).is_err());
}
#[test]
fn test_validate_setup_handoff_low_confidence_inferred_ready() {
    // Low confidence + inferred + ready = error
}
#[test]
fn test_validate_setup_handoff_not_ready_without_question() {
    assert!(validate_setup_handoff(&make_handoff(ready_to_launch: false, clarification_question: None)).is_err());
}

// GAP: slugify — edge cases not covered
#[test]
fn test_slugify_unicode() { /* unicode chars handled */ }
#[test]
fn test_slugify_all_special_chars() { /* all special -> hyphens -> collapse */ }
#[test]
fn test_slugify_single_char() { assert_eq!(slugify("A"), "a"); }
#[test]
fn test_slugify_numbers_only() { assert_eq!(slugify("123"), "123"); }
#[test]
fn test_slugify_leading_trailing_hyphens() { assert_eq!(slugify("-test-"), "test"); }
#[test]
fn test_slugify_consecutive_hyphens() { /* collapsed */ }

// === runtime.rs ===

// GAP: build_run_tag — timestamp format not tested
#[test]
fn test_build_run_tag_format() {
    let tag = build_run_tag();
    assert_eq!(tag.len(), 16); // "20260101T120000Z"
    assert!(tag.ends_with('Z'));
    assert!(tag.contains('T'));
}

// GAP: build_run_id — ID generation not tested
#[test]
fn test_build_run_id_format() {
    assert_eq!(build_run_id("My-Mission", "20260101T120000Z"), "my-mission-20260101t120000z");
}

// GAP: parse_evaluator_result — edge cases not tested
#[test]
fn test_parse_evaluator_result_whitespace_only() { assert!(parse_evaluator_result("   ").is_err()); }
#[test]
fn test_parse_evaluator_result_non_object() { assert!(parse_evaluator_result("[]").is_err()); }
#[test]
fn test_parse_evaluator_result_missing_pass() { assert!(parse_evaluator_result(r#"{"score":1.0}"#).is_err()); }
#[test]
fn test_parse_evaluator_result_wrong_pass_type() { assert!(parse_evaluator_result(r#"{"pass":"yes"}"#).is_err()); }
#[test]
fn test_parse_evaluator_result_null_pass() { assert!(parse_evaluator_result(r#"{"pass":null}"#).is_err()); }

// GAP: decide_outcome — core decision logic, missing branches
#[test]
fn test_decide_outcome_candidate_no_evaluation() {
    // Candidate with None evaluation -> Discard
}
#[test]
fn test_decide_outcome_candidate_ambiguous_evaluation() {
    // Candidate + Error evaluation -> Discard
}
#[test]
fn test_decide_outcome_score_improvement_equal_score() {
    // Same score + ScoreImprovement -> Discard
}
#[test]
fn test_decide_outcome_score_improvement_pass_no_score() {
    // Pass but no score + ScoreImprovement -> Ambiguous
}
#[test]
fn test_decide_outcome_all_candidate_statuses() {
    // Noop -> Noop, Interrupted -> Interrupted
}

// GAP: parse_candidate_artifact — completely untested
#[test]
fn test_parse_candidate_artifact_valid() {
    let json = r#"{"status":"candidate","candidate_commit":"abc","base_commit":"base","description":"t","notes":[],"created_at":"2026-01-01T00:00:00Z"}"#;
    assert!(parse_candidate_artifact(json).is_ok());
}
#[test]
fn test_parse_candidate_artifact_missing_base_commit() { assert!(parse_candidate_artifact(r#"{"base_commit":""#).is_err()); }
#[test]
fn test_parse_candidate_artifact_whitespace_base_commit() { assert!(parse_candidate_artifact(r#"{"base_commit":"   "}"#).is_err()); }
#[test]
fn test_parse_candidate_artifact_all_statuses() {
    for s in ["candidate","noop","abort","interrupted"] { /* each parses */ }
}

// GAP: run_evaluator — untested (complex async, shell invocation)
#[tokio::test]
async fn test_run_evaluator_empty_command() { /* validate_evaluator_command path */ }

// GAP: OrchestratorConfig — untested
#[test]
fn test_orchestrator_config_default() {
    let c = OrchestratorConfig::default();
    assert_eq!(c.max_iterations, 100);
}
```

---

### omc-notifications (~33% coverage — 5 untested public functions)

```rust
// GAP: lib::notify — untested
#[tokio::test]
async fn test_notify_disabled() { /* disabled config -> noop */ }
#[tokio::test]
async fn test_notify_slack() { /* dispatches to slack.rs */ }

// GAP: slack::send — untested (private fn, but error paths matter)
#[tokio::test]
async fn test_send_webhook_failure() { /* network error -> Err */ }

// GAP: tmux::current_session — untested
#[test]
fn test_current_session_no_tmux() { /* None when not in tmux */ }

// GAP: tmux::current_pane_id — untested
#[test]
fn test_current_pane_id_no_tmux() { /* None when not in tmux */ }

// GAP: tmux::capture_pane — untested
#[tokio::test]
async fn test_capture_pane_no_tmux() { /* returns Err */ }

// GAP: template::compute_variables — untested
#[test]
fn test_compute_variables() { /* extracts variables from SetupHandoff */ }
```

---

### omc-mcp (~25% coverage — 9 untested functions)

```rust
// GAP: lib::all_tools — untested
#[test]
fn test_all_tools_returns_all() { assert!(!all_tools().is_empty()); }

// GAP: state_tools — all impl methods untested
#[test]
fn test_state_read_tool() { /* JSON-RPC read */ }
#[test]
fn test_state_write_tool() { /* JSON-RPC write */ }
#[test]
fn test_state_clear_tool() { /* JSON-RPC clear */ }
#[test]
fn test_state_list_active_tool() { /* returns active states */ }
#[test]
fn test_state_get_status_tool() { /* returns status */ }

// GAP: notepad_tools — all impl methods untested
#[test]
fn test_notepad_read_tool() { /* JSON-RPC read */ }
#[test]
fn test_notepad_write_priority_tool() { /* priority ordering */ }
#[test]
fn test_notepad_write_working_tool() { /* working content */ }
#[test]
fn test_notepad_write_manual_tool() { /* manual content */ }

// GAP: memory_tools — all impl methods untested
#[test]
fn test_project_memory_read_tool() { /* JSON-RPC read */ }
#[test]
fn test_project_memory_write_tool() { /* JSON-RPC write */ }
#[test]
fn test_project_memory_add_note_tool() { /* appends note */ }
#[test]
fn test_project_memory_add_directive_tool() { /* adds directive */ }

// GAP: new_shared — untested
#[tokio::test]
async fn test_new_shared() { /* creates shared protocol registry */ }
```

---

### omc-xcmd (~14% coverage — 4 untested functions)

```rust
// GAP: executor::run_xcmd — error paths not tested
#[test]
fn test_run_xcmd_script_not_found() { /* .x-cmd.root/X missing */ }
#[test]
fn test_run_xcmd_command_failure() { /* non-zero exit */ }
#[test]
fn test_run_xcmd_with_args() { /* pass args through */ }

// GAP: executor::count_packages — edge cases not tested
#[test]
fn test_count_packages_missing_lock_dir() { assert_eq!(count_packages(), Some(0)); }

// GAP: executor::list_packages — untested
#[test]
fn test_list_packages_empty() { /* empty vec */ }
#[test]
fn test_list_packages_read_error() { /* error propagation */ }

// NOTE: lib.rs is missing implementations for xcmd_root(), skills_dir(),
// agents_dir(), is_installed(), get_version() — tests reference these but
// they are not defined. This is a compile error that must be fixed.
```

---

### omc-cli (~19% coverage — 15+ untested functions)

```rust
// GAP: dispatch::run — untested (complex, requires omc_team integration)
#[tokio::test]
async fn test_run_setup_host() { /* omc setup --host claude */ }
#[tokio::test]
async fn test_run_setup_host_invalid() { /* unknown host kind */ }
#[tokio::test]
async fn test_run_template_not_found() { /* DispatchError::NotFound */ }

// GAP: dispatch::skill_name — all 30 command variants
#[test]
fn test_skill_name_all_variants() { /* each Commands -> Option<&str> */ }

// GAP: dispatch::skill_args — all command variants
#[test]
fn test_skill_args_all_variants() { /* each Commands -> Option<SkillArgs> */ }

// GAP: dispatch::load_template — missing error paths
#[tokio::test]
async fn test_load_template_env_override() { /* OMC_SKILLS_DIR */ }
#[tokio::test]
async fn test_load_template_read_error() { /* file unreadable */ }

// GAP: dispatch::find_workspace_root — edge cases
#[test]
fn test_find_workspace_root_traversal() { /* deeply nested path */ }
#[test]
fn test_find_workspace_root_not_found() { /* no Cargo.toml */ }

// GAP: dispatch::omc_home — env override untested
#[test]
fn test_omc_home_env_override() { /* OMC_HOME */ }

// GAP: dispatch::list_skills — edge cases
#[test]
fn test_list_skills_empty() { /* no skills found */ }
#[test]
fn test_list_skills_long_name() { /* > 30 char names */ }

// GAP: dispatch::discover_skill_sources — error paths
#[test]
fn test_discover_skill_sources_with_files() { /* files ignored */ }
#[test]
fn test_discover_skill_sources_no_skill_md() { /* dir skipped */ }

// GAP: dispatch::extract_description — error path
#[test]
fn test_extract_description_read_error() { /* file unreadable */ }

// GAP: dispatch::DispatchError — error variants untested
#[test]
fn test_dispatch_error_display() { /* format strings */ }
#[test]
fn test_dispatch_error_io_from() { /* io::Error -> DispatchError::Io */ }

// GAP: commands::Cli — CLI parsing untested
#[test]
fn test_cli_parse_setup() { /* omc setup --host claude */ }
#[test]
fn test_cli_parse_skill() { /* omc skill arg1 arg2 */ }

// GAP: commands::SkillArgs — edge cases
#[test]
fn test_skill_args_special_chars() { /* quotes, backticks */ }
#[test]
fn test_skill_args_hyphen_values() { /* allow_hyphen_values */ }
```

---

## Integration Test Gaps

| Gap | Description | Priority |
|-----|-------------|----------|
| **Cross-crate orchestration** | omc-team + omc-git-provider integration (PR creation flow) | High |
| **Hook + skill lifecycle** | omc-hooks firing during omc-skills execution | High |
| **MCP bridge integration** | omc-interop with real MCP server | Medium |
| **Context injection** | omc-context + omc-host feeding context to agent | Medium |
| **Wiki + memory** | omc-wiki writes + omc-mcp memory_tools reads | Low |
| **CLI dispatch integration** | omc-cli full command execution with real templates | Medium |
| **Multi-provider git** | Detecting provider + parsing URL + invoking CLI end-to-end | High |

No integration tests exist outside `omc-skills/tests/integration.rs`.

---

## Error Handling Gaps (All Crates)

| Scenario | Crate(s) | Test Needed |
|----------|----------|-------------|
| Empty string inputs | git-provider, wiki, cli | `parse_remote_url("") -> None`, `tokenize("") -> vec![]` |
| Whitespace-only inputs | wiki, autoresearch, cli | `validate_setup_handoff` rejects them |
| Path traversal attempts | wiki | `write_page("../../../etc")` rejected |
| Reserved filenames | wiki | `write_page("index.md")` rejected |
| None/missing env vars | git-provider, cli | Missing credentials handled gracefully |
| Malformed YAML | wiki | `parse_frontmatter` returns None, doesn't panic |
| Invalid JSON from CLI | git-provider | All `view_pr`/`view_issue` error variants |
| NUL bytes in commands | autoresearch | `validate_evaluator_command` rejects |
| Network failures | notifications | Slack webhook timeout/failure |

---

## Missing Test Categories

| Category | Status | Notes |
|----------|--------|-------|
| Unit tests | 392 passing | Well-covered in omc-team, omc-hud, omc-shared |
| Integration tests | 1 file | Only omc-skills has integration tests |
| Property-based tests | None | Would be valuable for `slugify`, `tokenize`, `parse_frontmatter` |
| Fuzz tests | None | Would be valuable for YAML parsing, URL parsing |
| Benchmarks | None | No `benches/` directory |
| Error path tests | Sparse | Most crates lack negative test cases |
| Edge case tests | Sparse | Empty, whitespace, max-length cases often missing |

---

## Recommended Test Skeletons by Priority

### P0 — Critical (compile errors + critical paths)

1. `omc-xcmd/lib.rs` — implement missing functions or remove dangling test references
2. `omc-git-provider/github.rs` — `view_pr`/`view_issue` with invalid number
3. `omc-git-provider/lib.rs` — `parse_remote_url` edge cases (empty, azure, nested)
4. `omc-wiki/storage.rs` — `parse_frontmatter` + `serialize_page` (YAML core)

### P1 — High (main business logic)

5. `omc-wiki/wiki.rs` — `ingest_knowledge`, `query_wiki`, `lint_wiki` (3 main ops)
6. `omc-git-provider/types.rs` — `ProviderError` display, `ProviderName` display
7. `omc-autoresearch/runtime.rs` — `decide_outcome` (core decision logic)
8. `omc-autoresearch/runtime.rs` — `parse_candidate_artifact` (validation)
9. Cross-crate: git provider + team integration test

### P2 — Medium (important but lower risk)

10. `omc-notifications` — `notify`, `current_session`, `capture_pane`
11. `omc-mcp` — all 14 tool impl methods
12. `omc-autoresearch/prd.rs` — `validate_setup_handoff` edge cases
13. `omc-cli/dispatch.rs` — `run`, `skill_name`, `load_template`
14. Property-based: `slugify` (unicode, special chars), `tokenize` (CJK, mixed)

### P3 — Nice to have

15. `omc-xcmd/executor.rs` — error paths for `run_xcmd`
16. `omc-cli/commands.rs` — CLI parsing tests
17. `omc-git-provider` — all 5 provider `detect_from_remote`/`detect_from_api`
18. Fuzz tests for YAML frontmatter parsing
19. Fuzz tests for URL parsing in `parse_remote_url`
