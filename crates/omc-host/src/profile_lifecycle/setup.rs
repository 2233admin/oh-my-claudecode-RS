//! Idempotent generic profile registration with atomic configuration writes.

use super::ResolvedProfile;
use omc_shared::profile::{ConfigFormat, SETUP_SCHEMA_VERSION, SetupReport};
use serde_json::{Map, Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Default)]
pub struct SetupOptions {
    pub force: bool,
    pub print_only: bool,
}

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("profile has no setup descriptor")]
    MissingDescriptor,
    #[error("configuration conflict at {0}; use --force to replace it")]
    Conflict(String),
    #[error("configuration path is invalid: {0}")]
    InvalidPath(String),
    #[error("configuration I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration format failed: {0}")]
    Format(String),
}

pub fn setup_profile(
    resolved: &ResolvedProfile,
    project_root: &Path,
    options: SetupOptions,
) -> Result<SetupReport, SetupError> {
    let descriptor = resolved
        .profile
        .setup
        .as_ref()
        .ok_or(SetupError::MissingDescriptor)?;
    let destination = if descriptor.path.is_absolute() {
        descriptor.path.clone()
    } else {
        project_root.join(&descriptor.path)
    };
    let mut document = if destination.is_file() {
        parse(&fs::read_to_string(&destination)?, descriptor.format)?
    } else {
        Value::Object(Map::new())
    };
    let mut registration = Map::from_iter([
        ("command".into(), json!(resolved.profile.runtime.command)),
        ("args".into(), json!(resolved.profile.runtime.args)),
    ]);
    if !resolved.profile.runtime.environment.is_empty() {
        registration.insert("env".into(), json!(resolved.profile.runtime.environment));
    }
    let registration = Value::Object(registration);
    let existing = get_nested(&document, &descriptor.registration_path);
    if existing == Some(&registration) {
        return Ok(report(resolved, destination, false, None));
    }
    if existing.is_some() && !options.force {
        return Err(SetupError::Conflict(format!(
            "$.{}",
            descriptor.registration_path.join(".")
        )));
    }
    set_nested(&mut document, &descriptor.registration_path, registration)?;
    if options.print_only {
        return Ok(report(resolved, destination, true, None));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| SetupError::InvalidPath(destination.display().to_string()))?;
    fs::create_dir_all(parent)?;
    let backup = if destination.exists() && options.force {
        let backup = destination.with_extension(format!(
            "{}.omc-backup",
            destination
                .extension()
                .and_then(|item| item.to_str())
                .unwrap_or("config")
        ));
        fs::copy(&destination, &backup)?;
        Some(backup)
    } else {
        None
    };
    let serialized = serialize(&document, descriptor.format)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(serialized.as_bytes())?;
    temporary.flush()?;
    temporary
        .persist(&destination)
        .map_err(|error| SetupError::Io(error.error))?;
    Ok(report(resolved, destination, true, backup))
}

fn parse(content: &str, format: ConfigFormat) -> Result<Value, SetupError> {
    match format {
        ConfigFormat::Json => serde_json::from_str(content).map_err(|error| error.to_string()),
        ConfigFormat::Yaml => serde_yaml::from_str(content).map_err(|error| error.to_string()),
        ConfigFormat::Toml => toml::from_str::<toml::Value>(content)
            .map_err(|error| error.to_string())
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string())),
    }
    .map_err(SetupError::Format)
}

fn serialize(document: &Value, format: ConfigFormat) -> Result<String, SetupError> {
    match format {
        ConfigFormat::Json => serde_json::to_string_pretty(document).map_err(|e| e.to_string()),
        ConfigFormat::Yaml => serde_yaml::to_string(document).map_err(|e| e.to_string()),
        ConfigFormat::Toml => serde_json::from_value::<toml::Value>(document.clone())
            .map_err(|e| e.to_string())
            .and_then(|value| toml::to_string_pretty(&value).map_err(|e| e.to_string())),
    }
    .map(|mut text| {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text
    })
    .map_err(SetupError::Format)
}

fn get_nested<'a>(document: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter()
        .try_fold(document, |current, key| current.get(key))
}

fn set_nested(document: &mut Value, path: &[String], value: Value) -> Result<(), SetupError> {
    let (leaf, parents) = path
        .split_last()
        .ok_or_else(|| SetupError::InvalidPath("registrationPath is empty".into()))?;
    let mut current = document;
    for key in parents {
        let object = current
            .as_object_mut()
            .ok_or_else(|| SetupError::Conflict(format!("$.{}", parents.join("."))))?;
        current = object
            .entry(key.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    current
        .as_object_mut()
        .ok_or_else(|| SetupError::Conflict(format!("$.{}", parents.join("."))))?
        .insert(leaf.clone(), value);
    Ok(())
}

fn report(
    resolved: &ResolvedProfile,
    destination: PathBuf,
    changed: bool,
    backup: Option<PathBuf>,
) -> SetupReport {
    SetupReport {
        schema_version: SETUP_SCHEMA_VERSION.into(),
        profile_id: resolved.profile.id.clone(),
        changed,
        destination,
        provenance: resolved.provenance.clone(),
        backup,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_lifecycle::{ProfileRef, ResolutionContext, resolve_profile};
    use std::collections::BTreeMap;

    #[test]
    fn json_setup_preserves_siblings_and_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let profile_path = root.path().join("profile.json");
        fs::write(
            &profile_path,
            serde_json::to_vec(&json!({
                "schemaVersion":"omc.profile.v1", "id":"demo",
                "runtime":{"id":"r","command":"agent","args":["serve"]},
                "provider":{"id":"p"}, "model":{"id":"m"},
                "protocol":{"kind":"mcp-stdio"},
                "setup":{"format":"json","path":"consumer.json","registrationPath":["mcpServers","demo"]}
            })).unwrap(),
        ).unwrap();
        fs::write(root.path().join("consumer.json"), r#"{"keep":true}"#).unwrap();
        let resolved = resolve_profile(
            &ProfileRef::Explicit(profile_path),
            &ResolutionContext {
                project_root: root.path().into(),
                user_home: root.path().into(),
                organization_catalog: None,
                built_ins: BTreeMap::new(),
            },
        )
        .unwrap();
        assert!(
            setup_profile(&resolved, root.path(), SetupOptions::default())
                .unwrap()
                .changed
        );
        assert!(
            !setup_profile(&resolved, root.path(), SetupOptions::default())
                .unwrap()
                .changed
        );
        let written: Value =
            serde_json::from_slice(&fs::read(root.path().join("consumer.json")).unwrap()).unwrap();
        assert_eq!(written["keep"], true);
        assert_eq!(written["mcpServers"]["demo"]["command"], "agent");
    }
}
