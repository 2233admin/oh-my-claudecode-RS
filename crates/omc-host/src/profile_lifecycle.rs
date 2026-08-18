//! Resolution and validation seam for universal runtime profiles.

use omc_shared::profile::{
    ContractIssue, PROFILE_SCHEMA_VERSION, Profile, ProfileSource, ProtocolDescriptor, Provenance,
    VALIDATION_SCHEMA_VERSION, ValidationReport,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

mod probe;
pub use probe::probe_stdio;
mod setup;
pub use setup::{SetupError, SetupOptions, setup_profile};
mod doctor;
pub use doctor::doctor_profile;

#[derive(Debug, Clone)]
pub enum ProfileRef {
    Explicit(PathBuf),
    Named(String),
}

#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub project_root: PathBuf,
    pub user_home: PathBuf,
    pub organization_catalog: Option<PathBuf>,
    pub built_ins: BTreeMap<String, Profile>,
}

#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    pub profile: Profile,
    pub provenance: Provenance,
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile not found: {0}")]
    NotFound(String),
    #[error("ambiguous profile '{id}' at {level:?}: {paths:?}")]
    Ambiguous {
        id: String,
        level: ProfileSource,
        paths: Vec<PathBuf>,
    },
    #[error("cannot read profile {path}: {message}")]
    Read { path: PathBuf, message: String },
    #[error("cannot parse profile {path}: {message}")]
    Parse { path: PathBuf, message: String },
}

pub fn resolve_profile(
    reference: &ProfileRef,
    context: &ResolutionContext,
) -> Result<ResolvedProfile, ProfileError> {
    if let ProfileRef::Explicit(path) = reference {
        return load_file(path, ProfileSource::Explicit);
    }
    let ProfileRef::Named(id) = reference else {
        unreachable!();
    };
    let levels = [
        (
            ProfileSource::Project,
            Some(context.project_root.join(".omc/profiles")),
        ),
        (
            ProfileSource::User,
            Some(context.user_home.join("profiles")),
        ),
        (
            ProfileSource::Organization,
            context.organization_catalog.clone(),
        ),
    ];
    for (source, directory) in levels {
        let Some(directory) = directory else { continue };
        let candidates = profile_candidates(&directory, id);
        if candidates.len() > 1 {
            return Err(ProfileError::Ambiguous {
                id: id.clone(),
                level: source,
                paths: candidates,
            });
        }
        if let Some(path) = candidates.first() {
            return load_file(path, source);
        }
    }
    let profile = context
        .built_ins
        .get(id)
        .cloned()
        .ok_or_else(|| ProfileError::NotFound(id.clone()))?;
    let bytes = serde_json::to_vec(&profile).map_err(|error| ProfileError::Parse {
        path: PathBuf::from(format!("builtin:{id}")),
        message: error.to_string(),
    })?;
    Ok(ResolvedProfile {
        provenance: Provenance {
            source: ProfileSource::BuiltIn,
            schema_version: profile.schema_version.clone(),
            digest: digest(&bytes),
            catalog_version: None,
            location: Some(PathBuf::from(format!("builtin:{id}"))),
        },
        profile,
    })
}

pub fn validate_profile(resolved: ResolvedProfile) -> ValidationReport {
    let mut issues = Vec::new();
    if resolved.profile.schema_version != PROFILE_SCHEMA_VERSION {
        issues.push(issue(
            "migration_required",
            "$.schemaVersion",
            format!(
                "unsupported profile schema {}; expected {PROFILE_SCHEMA_VERSION}",
                resolved.profile.schema_version
            ),
        ));
    }
    validate_identifier("$.id", &resolved.profile.id, &mut issues);
    validate_identifier("$.runtime.id", &resolved.profile.runtime.id, &mut issues);
    validate_identifier("$.provider.id", &resolved.profile.provider.id, &mut issues);
    validate_identifier("$.model.id", &resolved.profile.model.id, &mut issues);
    if resolved.profile.runtime.command.as_os_str().is_empty() {
        issues.push(issue("required", "$.runtime.command", "command is empty"));
    }
    for (name, value) in &resolved.profile.runtime.environment {
        if looks_secret(name) || looks_secret(value) {
            issues.push(issue(
                "secret_value_forbidden",
                format!("$.runtime.environment.{name}"),
                "profiles may reference secret names but may not contain secret values",
            ));
        }
    }
    if let ProtocolDescriptor::McpHttpSse { endpoint } = &resolved.profile.protocol {
        let valid = reqwest::Url::parse(endpoint).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        });
        if !valid {
            issues.push(issue(
                "invalid_endpoint",
                "$.protocol.endpoint",
                "HTTP/SSE endpoint must be an absolute http(s) URL without credentials",
            ));
        }
    }
    for key in resolved.profile.extensions.keys() {
        if !key.contains('/') {
            issues.push(issue(
                "extension_not_namespaced",
                format!("$.extensions.{key}"),
                "extension keys must contain an owner namespace",
            ));
        }
    }
    ValidationReport {
        schema_version: VALIDATION_SCHEMA_VERSION.into(),
        valid: issues.is_empty(),
        profile: issues.is_empty().then_some(resolved.profile),
        provenance: Some(resolved.provenance),
        issues,
    }
}

fn profile_candidates(directory: &Path, id: &str) -> Vec<PathBuf> {
    ["json", "yaml", "yml", "toml"]
        .into_iter()
        .map(|extension| directory.join(format!("{id}.{extension}")))
        .filter(|path| path.is_file())
        .collect()
}

fn load_file(path: &Path, source: ProfileSource) -> Result<ResolvedProfile, ProfileError> {
    let bytes = fs::read(path).map_err(|error| ProfileError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let profile: Profile = match extension {
        "yaml" | "yml" => serde_yaml::from_slice(&bytes).map_err(|error| error.to_string()),
        "toml" => std::str::from_utf8(&bytes)
            .map_err(|error| error.to_string())
            .and_then(|text| toml::from_str(text).map_err(|error| error.to_string())),
        _ => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
    }
    .map_err(|message| ProfileError::Parse {
        path: path.to_path_buf(),
        message,
    })?;
    Ok(ResolvedProfile {
        provenance: Provenance {
            source,
            schema_version: profile.schema_version.clone(),
            digest: digest(&bytes),
            catalog_version: None,
            location: Some(path.to_path_buf()),
        },
        profile,
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_identifier(path: &str, value: &str, issues: &mut Vec<ContractIssue>) {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._/:".contains(character))
    {
        issues.push(issue(
            "invalid_identifier",
            path,
            "identifier must be a non-empty portable open string",
        ));
    }
}

fn looks_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("secret")
        || lower.contains("token=")
        || lower.starts_with("sk-")
}

fn issue(code: &str, path: impl Into<String>, message: impl Into<String>) -> ContractIssue {
    ContractIssue {
        code: code.into(),
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(id: &str) -> Profile {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": PROFILE_SCHEMA_VERSION,
            "id": id,
            "runtime": {"id": "runtime", "command": "agent"},
            "provider": {"id": "provider"},
            "model": {"id": "model"},
            "protocol": {"kind": "mcp-stdio"}
        }))
        .unwrap()
    }

    #[test]
    fn project_profile_precedes_user_and_builtin() {
        let root = tempfile::tempdir().unwrap();
        let user = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".omc/profiles")).unwrap();
        fs::create_dir_all(user.path().join("profiles")).unwrap();
        fs::write(
            root.path().join(".omc/profiles/demo.json"),
            serde_json::to_vec(&fixture("project-demo")).unwrap(),
        )
        .unwrap();
        fs::write(
            user.path().join("profiles/demo.json"),
            serde_json::to_vec(&fixture("user-demo")).unwrap(),
        )
        .unwrap();
        let resolved = resolve_profile(
            &ProfileRef::Named("demo".into()),
            &ResolutionContext {
                project_root: root.path().into(),
                user_home: user.path().into(),
                organization_catalog: None,
                built_ins: [("demo".into(), fixture("builtin-demo"))].into(),
            },
        )
        .unwrap();
        assert_eq!(resolved.profile.id, "project-demo");
        assert_eq!(resolved.provenance.source, ProfileSource::Project);
        assert_eq!(resolved.provenance.digest.len(), 64);
    }

    #[test]
    fn validation_rejects_major_and_embedded_secret() {
        let mut profile = fixture("demo");
        profile.schema_version = "omc.profile.v2".into();
        profile
            .runtime
            .environment
            .insert("API_KEY".into(), "sk-not-allowed".into());
        let report = validate_profile(ResolvedProfile {
            provenance: Provenance {
                source: ProfileSource::Explicit,
                schema_version: profile.schema_version.clone(),
                digest: "a".repeat(64),
                catalog_version: None,
                location: None,
            },
            profile,
        });
        assert!(!report.valid);
        assert!(
            report
                .issues
                .iter()
                .any(|item| item.code == "migration_required")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|item| item.code == "secret_value_forbidden")
        );
    }

    #[test]
    fn validation_rejects_malformed_or_credentialed_endpoint() {
        let mut profile = fixture("demo");
        profile.protocol = ProtocolDescriptor::McpHttpSse {
            endpoint: "https://user:password@example.invalid/mcp".into(),
        };
        let report = validate_profile(ResolvedProfile {
            provenance: Provenance {
                source: ProfileSource::Explicit,
                schema_version: profile.schema_version.clone(),
                digest: "a".repeat(64),
                catalog_version: None,
                location: None,
            },
            profile,
        });
        assert!(
            report
                .issues
                .iter()
                .any(|item| item.code == "invalid_endpoint")
        );
    }
}
