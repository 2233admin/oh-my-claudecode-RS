//! Thin CLI projection of the universal profile lifecycle.

use super::DispatchError;
use crate::commands::ProfileCommand;
use omc_host::profile_lifecycle::{
    ProfileRef, ResolutionContext, SetupOptions, doctor_profile, resolve_profile, setup_profile,
    validate_profile,
};
use omc_host::protocol_adapter::probe_protocol;
use omc_shared::OmcPaths;
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn run_profile(command: &ProfileCommand, root: &Path) -> Result<(), DispatchError> {
    match command {
        ProfileCommand::Init { id, output } => init_profile(id, output),
        ProfileCommand::List { json } => list_profiles(root, *json),
        ProfileCommand::Validate { profile, json } => {
            let resolved = resolve_explicit(profile, root)?;
            let report = validate_profile(resolved);
            emit(&report, *json)?;
            if report.valid {
                Ok(())
            } else {
                Err(DispatchError::Profile("profile validation failed".into()))
            }
        }
    }
}

pub fn run_probe(
    path: &Path,
    root: &Path,
    timeout_ms: u64,
    json_output: bool,
) -> Result<(), DispatchError> {
    let resolved = resolve_valid(path, root)?;
    let report = probe_protocol(&resolved, Duration::from_millis(timeout_ms))
        .map_err(|error| DispatchError::Profile(error.to_string()))?;
    emit(&report, json_output)?;
    if report.ready {
        Ok(())
    } else {
        Err(DispatchError::Profile("probe not ready".into()))
    }
}

pub fn run_profile_setup(
    path: &Path,
    root: &Path,
    force: bool,
    print_only: bool,
    json_output: bool,
) -> Result<(), DispatchError> {
    let resolved = resolve_valid(path, root)?;
    let report = setup_profile(&resolved, root, SetupOptions { force, print_only })
        .map_err(|error| DispatchError::Profile(error.to_string()))?;
    emit(&report, json_output)
}

pub fn run_profile_doctor(
    path: &Path,
    root: &Path,
    json_output: bool,
) -> Result<(), DispatchError> {
    let resolved = resolve_valid(path, root)?;
    let report = doctor_profile(&resolved, root, Duration::from_secs(3));
    emit(&report, json_output)?;
    if report.ready {
        Ok(())
    } else {
        Err(DispatchError::Profile("doctor not ready".into()))
    }
}

fn resolve_explicit(
    path: &Path,
    root: &Path,
) -> Result<omc_host::profile_lifecycle::ResolvedProfile, DispatchError> {
    resolve_profile(&ProfileRef::Explicit(path.to_path_buf()), &context(root))
        .map_err(|error| DispatchError::Profile(error.to_string()))
}

fn resolve_valid(
    path: &Path,
    root: &Path,
) -> Result<omc_host::profile_lifecycle::ResolvedProfile, DispatchError> {
    let resolved = resolve_explicit(path, root)?;
    let report = validate_profile(resolved.clone());
    if report.valid {
        Ok(resolved)
    } else {
        Err(DispatchError::Profile(serde_json::to_string(
            &report.issues,
        )?))
    }
}

fn context(root: &Path) -> ResolutionContext {
    ResolutionContext {
        project_root: root.to_path_buf(),
        user_home: OmcPaths::new().home,
        organization_catalog: Some(
            std::env::var_os("OMC_ORG_PROFILE_DIR")
                .or_else(|| std::env::var_os("OMC_CATALOG_HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(".omc/catalogs")),
        ),
        built_ins: BTreeMap::new(),
    }
}

fn init_profile(id: &str, output: &Path) -> Result<(), DispatchError> {
    if output.exists() {
        return Err(DispatchError::Profile(format!(
            "{} already exists",
            output.display()
        )));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let template = json!({
        "schemaVersion":"omc.profile.v1", "id":id,
        "runtime":{"id":"replace-runtime","command":"replace-agent"},
        "provider":{"id":"replace-provider"}, "model":{"id":"replace-model"},
        "protocol":{"kind":"mcp-stdio"}
    });
    fs::write(output, serde_json::to_vec_pretty(&template)?)?;
    println!("{}", output.display());
    Ok(())
}

fn list_profiles(root: &Path, json_output: bool) -> Result<(), DispatchError> {
    let mut names = BTreeSet::new();
    for directory in [
        root.join(".omc/profiles"),
        OmcPaths::new().home.join("profiles"),
    ] {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Some(stem) = entry.path().file_stem().and_then(|item| item.to_str()) {
                names.insert(stem.to_string());
            }
        }
    }
    emit(&names, json_output)
}

fn emit<T: Serialize>(data: &T, json_output: bool) -> Result<(), DispatchError> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(&json!({"schemaVersion":"omc.cli.v1","ok":true,"data":data}))?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(data)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_refuses_to_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("profile.json");
        fs::write(&path, "existing").unwrap();
        assert!(init_profile("demo", &path).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "existing");
    }
}
