use super::*;
use omc_host::{HostDoctorReport, HostKind};
use omc_skills::SkillRegistrar;

pub(super) fn collect_doctor_reports(
    root: &Path,
    host: Option<&str>,
) -> Result<Vec<HostDoctorReport>, DispatchError> {
    let hosts = match host {
        Some(host) => vec![HostKind::parse(host).map_err(DispatchError::NotFound)?],
        None => vec![HostKind::Claude, HostKind::Codex],
    };

    Ok(hosts
        .into_iter()
        .map(|kind| omc_host::create_adapter(kind).doctor(root))
        .collect())
}

pub(super) fn run_doctor(root: &Path, host: Option<&str>, json: bool) -> Result<(), DispatchError> {
    let reports = collect_doctor_reports(root, host)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        println!("OMC Doctor");
        println!("Project root: {}\n", root.display());
        for report in &reports {
            let status = if report.ready { "READY" } else { "ISSUES" };
            println!("[{}] {}", status, report.host);
            for message in &report.messages {
                println!("  - {message}");
            }
            println!();
        }
    }

    if reports.iter().all(|report| report.ready) {
        Ok(())
    } else {
        Err(DispatchError::NotFound(
            "one or more requested hosts are not ready".into(),
        ))
    }
}

/// Execute the real host setup flow: init project dirs, bootstrap .omc/,
/// discover and register skill sources, generate Codex manifest if needed.
pub(super) fn run_setup_host(
    root: &Path,
    host: &str,
    force: bool,
    hermes_home: Option<&Path>,
) -> Result<(), DispatchError> {
    if host.eq_ignore_ascii_case("hermes") {
        let home = omc_host::mcp_reg::resolve_hermes_home(hermes_home);
        let changed = omc_host::mcp_reg::ensure_hermes_mcp_server_with_force(
            &home,
            &omc_host::mcp_reg::omc_server_definition(),
            force,
        )
        .map_err(DispatchError::Host)?;
        println!("Setting up OMC for Hermes MCP consumer");
        println!("Hermes home: {}", home.display());
        println!(
            "MCP server `omc-rs`: {}",
            if changed {
                "registered"
            } else {
                "already registered"
            }
        );
        return Ok(());
    }

    let host_kind = HostKind::parse(host).map_err(DispatchError::NotFound)?;

    println!("Setting up OMC for host: {host_kind}");
    println!("Project root: {}\n", root.display());

    // 1. Init host project structure (.claude/ or .codex/)
    let adapter = omc_host::create_adapter(host_kind);
    let init_report = adapter
        .init_project(root)
        .map_err(DispatchError::NotFound)?;
    println!("Host directories ({}):", host_kind.config_dir_name());
    for p in &init_report.created {
        println!("  + {}", p.display());
    }
    for p in &init_report.unchanged {
        println!("  = {} (exists)", p.display());
    }

    let mcp_server = omc_host::mcp_reg::omc_server_definition();
    let mcp_changed =
        omc_host::mcp_reg::ensure_mcp_server_with_force(root, host_kind, &mcp_server, force)
            .map_err(DispatchError::Host)?;
    println!(
        "MCP server `omc-rs`: {}",
        if mcp_changed {
            "registered"
        } else {
            "already registered"
        }
    );

    // 2. Bootstrap .omc/ directory structure
    omc_skills::bootstrap::bootstrap_omc_dir(root).map_err(DispatchError::Io)?;
    println!("\n.omc/ directory bootstrapped.");

    // 3. Discover skill sources in .omc/skills/
    let omc_skills_dir = root.join(".omc").join("skills");
    let sources = discover_skill_sources(&omc_skills_dir);

    // 4. Register skill sources into host skills directory
    let host_skills_dir = root.join(host_kind.config_dir_name()).join("skills");
    let registrar = SkillRegistrar::new(&host_skills_dir);

    if sources.is_empty() {
        println!(
            "\nNo skill sources found in {}. Add skills with `omc skill add`.",
            omc_skills_dir.display()
        );
    } else {
        println!("\nRegistering {} skill source(s):", sources.len());
        let result = registrar.register_all(&sources);
        for linked in &result.linked {
            println!("  linked: {}", linked.display());
        }
        for copied in &result.copied {
            println!("  copied: {}", copied.display());
        }
        for skipped in &result.skipped {
            println!("  exists: {}", skipped.display());
        }
        for (path, err) in &result.errors {
            println!("  error: {} — {err}", path.display());
        }

        // 5. For Codex: generate skills.toml manifest
        if host_kind == HostKind::Codex {
            let mut loader = omc_skills::SkillLoader::new(&host_skills_dir);
            if let Ok(discovered) = loader.discover_all() {
                let manifest = registrar.generate_codex_manifest(&discovered);
                let manifest_path = root.join(".codex").join("skills.toml");
                if force || !manifest_path.exists() {
                    std::fs::write(&manifest_path, &manifest).map_err(DispatchError::Io)?;
                    println!(
                        "\nGenerated Codex manifest: {} ({} skills)",
                        manifest_path.display(),
                        discovered.len()
                    );
                } else {
                    println!(
                        "\nCodex manifest exists (use --force to overwrite): {}",
                        manifest_path.display()
                    );
                }
            }
        }
    }

    println!("\nSetup complete for {host_kind}.");
    Ok(())
}

/// Discover skill source directories under `dir`.
///
/// Returns `(source_dir, link_name)` pairs for directories containing SKILL.md.
pub(super) fn discover_skill_sources(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut sources = Vec::new();
    if !dir.is_dir() {
        return sources;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return sources;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            sources.push((path, name));
        }
    }

    sources
}
