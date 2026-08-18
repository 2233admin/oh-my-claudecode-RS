//! Trusted, versioned metadata catalog lifecycle.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const CATALOG_SCHEMA_VERSION: &str = "omc.catalog.v1";
pub const BUNDLED_CATALOG: &str = include_str!("../../../catalogs/bundled-v1.json");

mod validation;
pub use validation::{check_same_major_compatible, validate_catalog};
mod dependencies;
pub use dependencies::{DependencyEvidence, dependency_evidence};
mod crypto;
use crypto::signature_matches;
pub use crypto::{CryptoError, keyed_signature};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogProfileMetadata {
    pub id: String,
    pub schema_version: String,
    #[serde(default)]
    pub permissions: Vec<omc_shared::profile::OmcPermission>,
    /// Complete data-only profile used by organization catalogs. Older
    /// metadata-only bundled entries remain compatible.
    #[serde(default)]
    pub profile: Option<omc_shared::profile::Profile>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrustedSourceRegistry {
    #[serde(default)]
    sources: Vec<String>,
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
    #[error("catalog network error: {0}")]
    Network(String),
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
            if let Some((catalog, source)) = self.recover_missing_active()? {
                return Ok((
                    catalog.clone(),
                    status(&catalog, source, self.root.join("previous.json").is_file())?,
                ));
            }
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

    /// Complete or roll back an activation interrupted after active was moved
    /// aside. A valid staged catalog wins because it represents the fully
    /// validated transaction being committed; previous is the safe fallback.
    fn recover_missing_active(&self) -> Result<Option<(Catalog, &'static str)>, CatalogError> {
        let staged = self.root.join("staged.json");
        let previous = self.root.join("previous.json");
        for (candidate, source) in [
            (&staged, "recovered-staged"),
            (&previous, "recovered-previous"),
        ] {
            if !candidate.is_file() {
                continue;
            }
            if let Ok(catalog) = self.read_valid(candidate) {
                fs::rename(candidate, self.root.join("active.json"))?;
                return Ok(Some((catalog, source)));
            }
        }
        Ok(None)
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
        self.verify_and_activate(&fs::read(source)?, trust_key)
    }

    /// Persist an explicitly approved source. Registration never downloads or
    /// evaluates catalog content.
    pub fn trust_source(&self, source: &str) -> Result<(), CatalogError> {
        let source = normalize_source(source)?;
        fs::create_dir_all(&self.root)?;
        let path = self.root.join("trusted-sources.json");
        let mut registry = if path.is_file() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            TrustedSourceRegistry::default()
        };
        if !registry.sources.contains(&source) {
            registry.sources.push(source);
            registry.sources.sort();
            atomic_write(&path, &serde_json::to_vec_pretty(&registry)?)?;
        }
        Ok(())
    }

    /// Refresh from a registered local path or HTTP(S) URI. Trust is checked
    /// before opening a file, connecting a socket, or issuing an HTTP request.
    pub fn refresh_source(
        &self,
        source: &str,
        trust_key: &str,
    ) -> Result<CatalogStatus, CatalogError> {
        let normalized = normalize_source(source)?;
        if !self.is_trusted(&normalized)? {
            return Err(CatalogError::UntrustedSource(source.into()));
        }
        let bytes = if normalized.starts_with("http://") || normalized.starts_with("https://") {
            let url = reqwest::Url::parse(&normalized)
                .map_err(|_| CatalogError::UntrustedSource(source.into()))?;
            let response = reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|error| CatalogError::Network(error.to_string()))?
                .get(url)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .map_err(|error| CatalogError::Network(error.to_string()))?;
            let mut bytes = Vec::new();
            response
                .take(4 * 1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(CatalogError::Io)?;
            bytes
        } else {
            fs::read(Path::new(&normalized))?
        };
        self.verify_and_activate(&bytes, trust_key)
    }

    fn is_trusted(&self, normalized: &str) -> Result<bool, CatalogError> {
        let path = self.root.join("trusted-sources.json");
        if !path.is_file() {
            return Ok(false);
        }
        let registry: TrustedSourceRegistry = serde_json::from_slice(&fs::read(path)?)?;
        Ok(registry.sources.iter().any(|source| source == normalized))
    }

    fn verify_and_activate(
        &self,
        bytes: &[u8],
        trust_key: &str,
    ) -> Result<CatalogStatus, CatalogError> {
        let envelope: SignedCatalog = serde_json::from_slice(bytes)?;
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
        if !signature_matches(trust_key, &digest, &envelope.signature) {
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

fn normalize_source(source: &str) -> Result<String, CatalogError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let url = reqwest::Url::parse(source)
            .map_err(|_| CatalogError::UntrustedSource(source.into()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(CatalogError::UntrustedSource(source.into()));
        }
        return Ok(url.to_string());
    }
    let path = Path::new(source);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_path_lexically(&absolute)
        .to_string_lossy()
        .into_owned())
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if normalized.file_name().is_some() => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CatalogError> {
    let staged = path.with_extension("staged");
    fs::write(&staged, bytes)?;
    if path.is_file() {
        fs::remove_file(path)?;
    }
    fs::rename(staged, path)?;
    Ok(())
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
#[cfg(test)]
mod tests;
