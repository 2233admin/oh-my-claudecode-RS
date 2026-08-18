//! Resolution and validation seam for universal runtime profiles.

use crate::catalog::CatalogManager;
use omc_shared::profile::{Profile, ProfileSource, Provenance};
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
mod validation;
pub use validation::validate_profile;

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
    #[error("invalid profile reference: {0}")]
    InvalidReference(String),
}

pub fn resolve_profile(
    reference: &ProfileRef,
    context: &ResolutionContext,
) -> Result<ResolvedProfile, ProfileError> {
    if let ProfileRef::Explicit(path) = reference {
        return load_file(path, ProfileSource::Explicit);
    }
    let id = match reference {
        ProfileRef::Named(id) => id,
        ProfileRef::Explicit(_) => {
            return Err(ProfileError::InvalidReference(
                "invalid explicit profile reference".into(),
            ));
        }
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
        let catalog_profile = (source == ProfileSource::Organization)
            .then(|| load_catalog_profile(&directory, id))
            .transpose()?
            .flatten();
        if candidates.len() + usize::from(catalog_profile.is_some()) > 1 {
            let mut paths = candidates;
            if catalog_profile.is_some() {
                paths.push(directory.join("active.json"));
            }
            return Err(ProfileError::Ambiguous {
                id: id.clone(),
                level: source,
                paths,
            });
        }
        if let Some(path) = candidates.first() {
            return load_file(path, source);
        }
        if let Some(resolved) = catalog_profile {
            return Ok(resolved);
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

fn load_catalog_profile(root: &Path, id: &str) -> Result<Option<ResolvedProfile>, ProfileError> {
    let manager = CatalogManager::new(root);
    let (catalog, status) = manager.active().map_err(|error| ProfileError::Read {
        path: root.to_path_buf(),
        message: error.to_string(),
    })?;
    let Some(profile) = catalog
        .profiles
        .iter()
        .find(|metadata| metadata.id == id)
        .and_then(|metadata| metadata.profile.clone())
    else {
        return Ok(None);
    };
    let bytes = serde_json::to_vec(&profile).map_err(|error| ProfileError::Parse {
        path: root.join("active.json"),
        message: error.to_string(),
    })?;
    Ok(Some(ResolvedProfile {
        provenance: Provenance {
            source: ProfileSource::Organization,
            schema_version: profile.schema_version.clone(),
            digest: digest(&bytes),
            catalog_version: Some(status.catalog_version),
            location: Some(if status.source == "bundled" {
                PathBuf::from("bundled:catalog")
            } else {
                root.join("active.json")
            }),
        },
        profile,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use omc_shared::profile::{PROFILE_SCHEMA_VERSION, ProtocolDescriptor};

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
    fn active_organization_catalog_resolves_profile_with_catalog_provenance() {
        use crate::catalog::{
            CatalogManager, CatalogProfileMetadata, SignedCatalog, keyed_signature,
        };

        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("catalog");
        let source = root.path().join("source.json");
        let manager = CatalogManager::new(&state);
        let profile = fixture("organization-demo");
        let mut catalog = CatalogManager::bundled().unwrap();
        catalog.catalog_version = "1.1.0".into();
        catalog.profiles.push(CatalogProfileMetadata {
            id: "organization-demo".into(),
            schema_version: PROFILE_SCHEMA_VERSION.into(),
            permissions: vec![],
            profile: Some(profile.clone()),
        });
        let value = serde_json::to_value(&catalog).unwrap();
        let bytes = serde_json::to_vec(&value).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let envelope = SignedCatalog {
            digest: digest.clone(),
            signature_algorithm: "hmac-sha256-v1".into(),
            signature: keyed_signature("key", &digest).unwrap(),
            catalog: value,
        };
        fs::write(&source, serde_json::to_vec(&envelope).unwrap()).unwrap();
        manager.refresh_file(&source, &source, "key").unwrap();

        let resolved = resolve_profile(
            &ProfileRef::Named("organization-demo".into()),
            &ResolutionContext {
                project_root: root.path().join("project"),
                user_home: root.path().join("user"),
                organization_catalog: Some(state.clone()),
                built_ins: BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(resolved.profile, profile);
        assert_eq!(resolved.provenance.source, ProfileSource::Organization);
        assert_eq!(
            resolved.provenance.catalog_version.as_deref(),
            Some("1.1.0")
        );
        assert_eq!(resolved.provenance.digest.len(), 64);
        assert_eq!(
            resolved.provenance.location.as_deref(),
            Some(state.join("active.json").as_path())
        );
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
