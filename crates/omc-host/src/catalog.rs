//! Trusted, versioned metadata catalog lifecycle.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const CATALOG_SCHEMA_VERSION: &str = "omc.catalog.v1";
pub const BUNDLED_CATALOG: &str = include_str!("../../../catalogs/bundled-v1.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: String,
    pub catalog_version: String,
    pub compatible_omc: VersionRange,
    pub compatible_profile_schema: Vec<String>,
    #[serde(default)]
    pub profiles: Vec<CatalogProfileMetadata>,
    #[serde(default)]
    pub providers: Vec<CatalogProviderMetadata>,
    #[serde(default)]
    pub models: Vec<CatalogModelMetadata>,
    #[serde(default)]
    pub dependencies: Vec<DependencyMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProfileMetadata {
    pub id: String,
    pub schema_version: String,
    #[serde(default)]
    pub permissions: Vec<omc_shared::profile::OmcPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProviderMetadata {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogModelMetadata {
    pub id: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionRange {
    pub min_inclusive: String,
    pub max_exclusive: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyMetadata {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedCatalog {
    pub digest: String,
    pub signature_algorithm: String,
    pub signature: String,
    pub catalog: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogStatus {
    pub schema_version: String,
    pub catalog_version: String,
    pub source: String,
    pub digest: String,
    pub rollback_available: bool,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("catalog JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("untrusted catalog source: {0}")]
    UntrustedSource(String),
    #[error("catalog integrity check failed: {0}")]
    Integrity(String),
    #[error("catalog is incompatible: {0}")]
    Incompatible(String),
    #[error("catalog schema rejected: {0}")]
    Schema(String),
    #[error("no previous catalog is available for rollback")]
    NoRollback,
}

pub struct CatalogManager {
    root: PathBuf,
    omc_version: String,
}

impl CatalogManager {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            omc_version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn bundled() -> Result<Catalog, CatalogError> {
        let catalog: Catalog = serde_json::from_str(BUNDLED_CATALOG)?;
        validate_catalog(&catalog, env!("CARGO_PKG_VERSION"))?;
        Ok(catalog)
    }

    pub fn active(&self) -> Result<(Catalog, CatalogStatus), CatalogError> {
        let active = self.root.join("active.json");
        if !active.is_file() {
            let catalog = Self::bundled()?;
            return Ok((catalog.clone(), status(&catalog, "bundled", false)?));
        }
        let catalog = match self.read_valid(&active) {
            Ok(catalog) => catalog,
            Err(active_error) => {
                let previous = self.root.join("previous.json");
                let recovered = self.read_valid(&previous).map_err(|_| active_error)?;
                let recovery = self.root.join("recovered.json");
                let quarantined = self.root.join("corrupt-active.json");
                fs::write(&recovery, serde_json::to_vec_pretty(&recovered)?)?;
                if quarantined.is_file() {
                    fs::remove_file(&quarantined)?;
                }
                fs::rename(&active, &quarantined)?;
                fs::rename(&recovery, &active)?;
                recovered
            }
        };
        Ok((
            catalog.clone(),
            status(
                &catalog,
                "active",
                self.root.join("previous.json").is_file(),
            )?,
        ))
    }

    fn read_valid(&self, path: &Path) -> Result<Catalog, CatalogError> {
        let catalog: Catalog = serde_json::from_slice(&fs::read(path)?)?;
        validate_catalog(&catalog, &self.omc_version)?;
        Ok(catalog)
    }

    pub fn refresh_file(
        &self,
        source: &Path,
        trusted_source: &Path,
        trust_key: &str,
    ) -> Result<CatalogStatus, CatalogError> {
        let canonical_source = source.canonicalize()?;
        let canonical_trusted = trusted_source.canonicalize()?;
        if canonical_source != canonical_trusted {
            return Err(CatalogError::UntrustedSource(source.display().to_string()));
        }
        let envelope: SignedCatalog = serde_json::from_slice(&fs::read(source)?)?;
        if envelope.signature_algorithm != "hmac-sha256-v1" {
            return Err(CatalogError::Integrity(
                "unsupported signature algorithm".into(),
            ));
        }
        let catalog_bytes = serde_json::to_vec(&envelope.catalog)?;
        let digest = sha256_hex(&catalog_bytes);
        if digest != envelope.digest {
            return Err(CatalogError::Integrity("digest mismatch".into()));
        }
        if keyed_signature(trust_key, &digest) != envelope.signature {
            return Err(CatalogError::Integrity("signature mismatch".into()));
        }
        let catalog: Catalog = serde_json::from_value(envelope.catalog)?;
        validate_catalog(&catalog, &self.omc_version)?;
        let (current, _) = self.active()?;
        check_same_major_compatible(&current, &catalog)?;
        self.activate(&catalog)
    }

    pub fn rollback(&self) -> Result<CatalogStatus, CatalogError> {
        let previous = self.root.join("previous.json");
        if !previous.is_file() {
            return Err(CatalogError::NoRollback);
        }
        fs::create_dir_all(&self.root)?;
        let active = self.root.join("active.json");
        let swap = self.root.join("rollback-swap.json");
        if active.is_file() {
            fs::rename(&active, &swap)?;
        }
        fs::rename(&previous, &active)?;
        if swap.is_file() {
            fs::rename(&swap, &previous)?;
        }
        let (catalog, _) = self.active()?;
        status(
            &catalog,
            "rollback",
            self.root.join("previous.json").is_file(),
        )
    }

    fn activate(&self, catalog: &Catalog) -> Result<CatalogStatus, CatalogError> {
        fs::create_dir_all(&self.root)?;
        let staged = self.root.join("staged.json");
        let active = self.root.join("active.json");
        let previous = self.root.join("previous.json");
        let bytes = serde_json::to_vec_pretty(catalog)?;
        fs::write(&staged, &bytes)?;
        let reread: Catalog = serde_json::from_slice(&fs::read(&staged)?)?;
        validate_catalog(&reread, &self.omc_version)?;
        if active.is_file() {
            if previous.is_file() {
                fs::remove_file(&previous)?;
            }
            fs::rename(&active, &previous)?;
        }
        if let Err(error) = fs::rename(&staged, &active) {
            if previous.is_file() && !active.is_file() {
                let _ = fs::rename(&previous, &active);
            }
            return Err(error.into());
        }
        status(catalog, "active", previous.is_file())
    }
}

pub fn validate_catalog(catalog: &Catalog, omc_version: &str) -> Result<(), CatalogError> {
    if catalog.schema_version != CATALOG_SCHEMA_VERSION {
        return Err(CatalogError::Schema(format!(
            "unsupported {}",
            catalog.schema_version
        )));
    }
    if !catalog
        .compatible_profile_schema
        .iter()
        .any(|v| v == omc_shared::PROFILE_SCHEMA_VERSION)
    {
        return Err(CatalogError::Incompatible(
            "profile schema range excludes this binary".into(),
        ));
    }
    if !version_in_range(omc_version, &catalog.compatible_omc) {
        return Err(CatalogError::Incompatible(format!(
            "OMC {omc_version} requires an upgrade or older catalog"
        )));
    }
    Ok(())
}

pub fn check_same_major_compatible(previous: &Catalog, next: &Catalog) -> Result<(), CatalogError> {
    if major(&previous.schema_version) != major(&next.schema_version) {
        return Ok(());
    }
    for profile in &previous.profiles {
        let Some(candidate) = next.profiles.iter().find(|item| item.id == profile.id) else {
            return Err(CatalogError::Incompatible(format!(
                "removed profile {}",
                profile.id
            )));
        };
        if candidate.schema_version != profile.schema_version {
            return Err(CatalogError::Incompatible(format!(
                "changed profile schema type for {}",
                profile.id
            )));
        }
        if candidate
            .permissions
            .iter()
            .any(|permission| !profile.permissions.contains(permission))
        {
            return Err(CatalogError::Incompatible(format!(
                "permission expansion for {}",
                profile.id
            )));
        }
    }
    for model in &previous.models {
        let Some(candidate) = next.models.iter().find(|item| item.id == model.id) else {
            return Err(CatalogError::Incompatible(format!(
                "removed model {}",
                model.id
            )));
        };
        if model
            .capabilities
            .iter()
            .any(|capability| !candidate.capabilities.contains(capability))
        {
            return Err(CatalogError::Incompatible(format!(
                "narrowed capabilities for {}",
                model.id
            )));
        }
    }
    for dependency in &previous.dependencies {
        let Some(candidate) = next
            .dependencies
            .iter()
            .find(|item| item.id == dependency.id)
        else {
            return Err(CatalogError::Incompatible(format!(
                "removed dependency {}",
                dependency.id
            )));
        };
        if candidate.commands.len() < dependency.commands.len() {
            return Err(CatalogError::Incompatible(format!(
                "narrowed commands for {}",
                dependency.id
            )));
        }
    }
    Ok(())
}

pub fn keyed_signature(key: &str, digest: &str) -> String {
    hmac_sha256_hex(key.as_bytes(), digest.as_bytes())
}

fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    const BLOCK: usize = 64;
    let normalized = if key.len() > BLOCK {
        Sha256::digest(key).to_vec()
    } else {
        key.to_vec()
    };
    let mut padded = [0_u8; BLOCK];
    padded[..normalized.len()].copy_from_slice(&normalized);
    let mut inner_pad = padded;
    let mut outer_pad = padded;
    inner_pad.iter_mut().for_each(|byte| *byte ^= 0x36);
    outer_pad.iter_mut().for_each(|byte| *byte ^= 0x5c);
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    format!("{:x}", outer.finalize())
}

fn status(
    catalog: &Catalog,
    source: &str,
    rollback_available: bool,
) -> Result<CatalogStatus, CatalogError> {
    Ok(CatalogStatus {
        schema_version: CATALOG_SCHEMA_VERSION.into(),
        catalog_version: catalog.catalog_version.clone(),
        source: source.into(),
        digest: sha256_hex(&serde_json::to_vec(catalog)?),
        rollback_available,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn major(schema: &str) -> Option<&str> {
    schema.rsplit_once('v').map(|(_, major)| major)
}
fn version_in_range(version: &str, range: &VersionRange) -> bool {
    parse_version(version) >= parse_version(&range.min_inclusive)
        && parse_version(version) < parse_version(&range.max_exclusive)
}
fn parse_version(value: &str) -> (u64, u64, u64) {
    let mut parts = value
        .trim_start_matches('v')
        .split(['.', '-'])
        .take(3)
        .map(|p| p.parse().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn signed(catalog: &Catalog, key: &str) -> SignedCatalog {
        let value = serde_json::to_value(catalog).unwrap();
        let digest = sha256_hex(&serde_json::to_vec(&value).unwrap());
        SignedCatalog {
            signature: keyed_signature(key, &digest),
            digest,
            signature_algorithm: "hmac-sha256-v1".into(),
            catalog: value,
        }
    }

    #[test]
    fn offline_baseline_contains_all_metadata_dimensions() {
        let catalog = CatalogManager::bundled().unwrap();
        assert!(
            !catalog.profiles.is_empty()
                && !catalog.providers.is_empty()
                && !catalog.models.is_empty()
                && !catalog.dependencies.is_empty()
        );
    }

    #[test]
    fn corrupt_or_untrusted_refresh_preserves_active_catalog() {
        let dir = tempdir().unwrap();
        let manager = CatalogManager::new(dir.path());
        let source = dir.path().join("source.json");
        fs::write(&source, "not-json").unwrap();
        assert!(manager.refresh_file(&source, &source, "key").is_err());
        assert_eq!(manager.active().unwrap().1.source, "bundled");
    }

    #[test]
    fn digest_mismatch_and_incompatible_range_are_rejected() {
        let dir = tempdir().unwrap();
        let manager = CatalogManager::new(dir.path().join("state"));
        let source = dir.path().join("source.json");
        let mut catalog = CatalogManager::bundled().unwrap();
        let mut envelope = signed(&catalog, "key");
        envelope.digest = "bad".into();
        fs::write(&source, serde_json::to_vec(&envelope).unwrap()).unwrap();
        assert!(matches!(
            manager.refresh_file(&source, &source, "key"),
            Err(CatalogError::Integrity(_))
        ));
        catalog.compatible_omc.min_inclusive = "99.0.0".into();
        fs::write(
            &source,
            serde_json::to_vec(&signed(&catalog, "key")).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            manager.refresh_file(&source, &source, "key"),
            Err(CatalogError::Incompatible(_))
        ));
    }

    #[test]
    fn activation_and_rollback_are_atomic_from_the_readers_view() {
        let dir = tempdir().unwrap();
        let manager = CatalogManager::new(dir.path().join("state"));
        let source = dir.path().join("source.json");
        let mut first = CatalogManager::bundled().unwrap();
        first.catalog_version = "1.1.0".into();
        fs::write(&source, serde_json::to_vec(&signed(&first, "key")).unwrap()).unwrap();
        manager.refresh_file(&source, &source, "key").unwrap();
        let mut second = first.clone();
        second.catalog_version = "1.2.0".into();
        fs::write(
            &source,
            serde_json::to_vec(&signed(&second, "key")).unwrap(),
        )
        .unwrap();
        manager.refresh_file(&source, &source, "key").unwrap();
        assert_eq!(manager.rollback().unwrap().catalog_version, "1.1.0");
    }

    #[test]
    fn interrupted_or_corrupt_active_pointer_recovers_previous() {
        let dir = tempdir().unwrap();
        let state = dir.path().join("state");
        let manager = CatalogManager::new(&state);
        let source = dir.path().join("source.json");
        let mut first = CatalogManager::bundled().unwrap();
        first.catalog_version = "1.1.0".into();
        fs::write(&source, serde_json::to_vec(&signed(&first, "key")).unwrap()).unwrap();
        manager.refresh_file(&source, &source, "key").unwrap();
        let mut second = first.clone();
        second.catalog_version = "1.2.0".into();
        fs::write(
            &source,
            serde_json::to_vec(&signed(&second, "key")).unwrap(),
        )
        .unwrap();
        manager.refresh_file(&source, &source, "key").unwrap();
        fs::write(state.join("active.json"), "interrupted").unwrap();
        assert_eq!(manager.active().unwrap().0.catalog_version, "1.1.0");
    }

    #[test]
    fn executable_directives_and_same_major_removal_fail_closed() {
        let mut value: Value = serde_json::from_str(BUNDLED_CATALOG).unwrap();
        value["profiles"][0]["installScript"] = Value::String("curl | sh".into());
        assert!(serde_json::from_value::<Catalog>(value).is_err());
        let previous = CatalogManager::bundled().unwrap();
        let mut next = previous.clone();
        next.dependencies.clear();
        assert!(check_same_major_compatible(&previous, &next).is_err());
    }
}
