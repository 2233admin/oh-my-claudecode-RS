//! Integration tests for skill registration + discovery flow.
//!
//! Tests the end-to-end lifecycle: register source dirs -> discover skills
//! through registered links -> host filtering -> collision resolution -> cleanup.

use omc_skills::{SkillLoader, SkillRegistrar};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Register individual skill directories from `source` into `host_skills` using
/// their directory name as the link name. Returns the number of successfully
/// registered skills.
///
/// This mirrors the pattern from the unit tests: each skill directory becomes a
/// direct entry in the host skills directory, which is what `SkillLoader` expects
/// when walking with `follow_links(false)`.
fn register_skills_from_source(registrar: &SkillRegistrar, source: &Path) -> usize {
    let mut count = 0;
    for entry in fs::read_dir(source)
        .unwrap()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            let link_name = entry.file_name().to_string_lossy().to_string();
            registrar.register(&path, &link_name).unwrap();
            count += 1;
        }
    }
    count
}

fn create_dir_skill(parent: &Path, name: &str, description: &str) {
    let skill_dir = parent.join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nContent for {name}."),
    )
    .unwrap();
}

fn create_flat_skill(parent: &Path, name: &str, description: &str) {
    fs::write(
        parent.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nContent for {name}."),
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Test 1: register source dirs -> discover skills through registered links
// ---------------------------------------------------------------------------

#[test]
fn test_register_then_discover_end_to_end() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Create source skill directories with different skills
    let global_skills = root.join("global-skills");
    let project_skills = root.join("project-skills");
    fs::create_dir_all(&global_skills).unwrap();
    fs::create_dir_all(&project_skills).unwrap();

    create_dir_skill(&global_skills, "tdd", "Test-driven development");
    create_dir_skill(
        &project_skills,
        "my-project-skill",
        "Project-specific skill",
    );

    // Create host skills directory
    let host_skills = root.join(".claude").join("skills");
    fs::create_dir_all(&host_skills).unwrap();

    // Register both sources (each skill directory gets its own link)
    let registrar = SkillRegistrar::new(&host_skills);
    register_skills_from_source(&registrar, &global_skills);
    register_skills_from_source(&registrar, &project_skills);

    // Discover skills
    let mut loader = SkillLoader::new(&host_skills);
    let skills = loader.discover_all().unwrap();

    assert_eq!(skills.len(), 2);
    assert!(skills.iter().any(|s| s.name == "tdd"));
    assert!(skills.iter().any(|s| s.name == "my-project-skill"));
}

// ---------------------------------------------------------------------------
// Test 2: host filtering — register skills with hosts field, filter by host
// ---------------------------------------------------------------------------

#[test]
fn test_host_filtering_after_registration() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let source = root.join("skills-src");
    fs::create_dir_all(&source).unwrap();

    // Skill available to all hosts (empty hosts)
    create_dir_skill(&source, "universal", "Works everywhere");
    // Skill only for claude
    create_dir_skill_with_hosts(&source, "claude-only", "Claude only", "[claude]");
    // Skill only for codex
    create_dir_skill_with_hosts(&source, "codex-only", "Codex only", "[codex]");

    let host_skills = root.join("host-skills");
    fs::create_dir_all(&host_skills).unwrap();

    let registrar = SkillRegistrar::new(&host_skills);
    register_skills_from_source(&registrar, &source);

    let mut loader = SkillLoader::new(&host_skills);
    let all_skills = loader.discover_all().unwrap();
    assert_eq!(
        all_skills.len(),
        3,
        "all three skills should be discoverable"
    );

    // No host filter returns everything
    let no_filter = loader.list_for_host(None);
    assert_eq!(no_filter.len(), 3);

    // Claude sees universal + claude-only
    let claude_skills = loader.list_for_host(Some("claude"));
    assert_eq!(claude_skills.len(), 2);
    assert!(claude_skills.iter().any(|s| s.name == "universal"));
    assert!(claude_skills.iter().any(|s| s.name == "claude-only"));

    // Codex sees universal + codex-only
    let codex_skills = loader.list_for_host(Some("codex"));
    assert_eq!(codex_skills.len(), 2);
    assert!(codex_skills.iter().any(|s| s.name == "universal"));
    assert!(codex_skills.iter().any(|s| s.name == "codex-only"));

    // Unknown host sees only universal
    let other_skills = loader.list_for_host(Some("opencode"));
    assert_eq!(other_skills.len(), 1);
    assert!(other_skills.iter().any(|s| s.name == "universal"));
}

// ---------------------------------------------------------------------------
// Test 3: collision resolution — same skill name in two sources, one wins
// ---------------------------------------------------------------------------

#[test]
fn test_collision_resolution_across_sources() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let source_a = root.join("source-a");
    let source_b = root.join("source-b");
    fs::create_dir_all(&source_a).unwrap();
    fs::create_dir_all(&source_b).unwrap();

    // Both sources have a skill named "common", with different descriptions
    create_dir_skill(&source_a, "common", "From source A");
    create_dir_skill(&source_b, "common", "From source B");

    let host_skills = root.join("host-skills");
    fs::create_dir_all(&host_skills).unwrap();

    let registrar = SkillRegistrar::new(&host_skills);
    register_skills_from_source(&registrar, &source_a);
    register_skills_from_source(&registrar, &source_b);

    let mut loader = SkillLoader::new(&host_skills);
    let skills = loader.discover_all().unwrap();

    // Exactly one "common" skill should survive (HashMap insert-overwrite
    // means the skill whose source directory is walked last wins)
    let common: Vec<_> = skills.iter().filter(|s| s.name == "common").collect();
    assert_eq!(
        common.len(),
        1,
        "collision should resolve to exactly one skill"
    );

    // The winner's description is either A or B depending on WalkDir iteration
    // order. The invariant we verify is that it resolved cleanly.
    assert!(
        common[0].description == "From source A" || common[0].description == "From source B",
        "winner should be one of the two sources"
    );
}

// ---------------------------------------------------------------------------
// Test 4: bootstrap + register + discover full lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_full_setup_lifecycle() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Step 1: Bootstrap host skills directory
    let host_skills = root.join(".omc").join("skills");
    fs::create_dir_all(&host_skills).unwrap();
    assert!(host_skills.exists());

    // Step 2: Create and register global skills (individual directory links)
    let global = root.join("global-skills");
    fs::create_dir_all(&global).unwrap();
    create_dir_skill(&global, "tdd", "Test-driven development");
    create_dir_skill(&global, "diagnose", "Bug diagnosis");

    let registrar = SkillRegistrar::new(&host_skills);
    let global_count = register_skills_from_source(&registrar, &global);
    assert_eq!(global_count, 2);

    // Step 3: Create and register project skill (directory)
    let project = root.join("project-skills");
    fs::create_dir_all(&project).unwrap();
    create_dir_skill(&project, "my-app", "My application skill");

    let project_count = register_skills_from_source(&registrar, &project);
    assert_eq!(project_count, 1);

    // Step 4: Add a flat-file skill directly to host_skills (simulates a
    // locally-defined skill that is not linked from an external source)
    create_flat_skill(&host_skills, "quick-ref", "Quick reference");

    // Step 5: Discover all skills — directory + flat
    let mut loader = SkillLoader::new(&host_skills);
    let skills = loader.discover_all().unwrap();

    assert_eq!(skills.len(), 4, "expected 3 dir skills + 1 flat skill");
    assert!(skills.iter().any(|s| s.name == "tdd"));
    assert!(skills.iter().any(|s| s.name == "diagnose"));
    assert!(skills.iter().any(|s| s.name == "my-app"));
    assert!(skills.iter().any(|s| s.name == "quick-ref"));

    // Step 6: Verify idempotent re-registration
    let r = registrar.register(&global.join("tdd"), "tdd").unwrap();
    assert_eq!(
        r.skipped.len(),
        1,
        "re-register should be skipped (idempotent)"
    );

    // Discovery still returns the same results
    let skills = loader.discover_all().unwrap();
    assert_eq!(skills.len(), 4);

    // Step 7: Verify registered entries
    let registered = registrar.list_registered();
    // Entries: tdd (link), diagnose (link), my-app (link), quick-ref.md (file) = 4
    assert_eq!(registered.len(), 4);
}

// ---------------------------------------------------------------------------
// Test 5: unregister removes link but source survives
// ---------------------------------------------------------------------------

#[test]
fn test_unregister_cleanup() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let source = root.join("skills-src");
    fs::create_dir_all(&source).unwrap();
    create_dir_skill(&source, "ephemeral", "To be removed");
    create_dir_skill(&source, "permanent", "Stays around");

    let host_skills = root.join("host-skills");
    fs::create_dir_all(&host_skills).unwrap();

    let registrar = SkillRegistrar::new(&host_skills);
    registrar
        .register(&source.join("ephemeral"), "ephemeral")
        .unwrap();
    registrar
        .register(&source.join("permanent"), "permanent")
        .unwrap();

    // Verify both are discoverable
    let mut loader = SkillLoader::new(&host_skills);
    let skills = loader.discover_all().unwrap();
    assert_eq!(skills.len(), 2);
    assert!(skills.iter().any(|s| s.name == "ephemeral"));
    assert!(skills.iter().any(|s| s.name == "permanent"));

    // Unregister one
    registrar.unregister("ephemeral").unwrap();

    // The link should be gone from host_skills
    assert!(!host_skills.join("ephemeral").exists());

    // The source directory must survive
    assert!(source.join("ephemeral").exists());
    assert!(source.join("ephemeral").join("SKILL.md").exists());

    // Discovery should now find only the remaining skill
    let skills = loader.discover_all().unwrap();
    assert_eq!(skills.len(), 1);
    assert!(skills.iter().any(|s| s.name == "permanent"));
    assert!(!skills.iter().any(|s| s.name == "ephemeral"));

    // Unregistering again is idempotent
    registrar.unregister("ephemeral").unwrap();

    // The other link is still intact
    assert!(host_skills.join("permanent").exists());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_dir_skill_with_hosts(parent: &Path, name: &str, description: &str, hosts_yaml: &str) {
    let skill_dir = parent.join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\nhosts: {hosts_yaml}\n---\n\nContent for {name}."),
    )
    .unwrap();
}
