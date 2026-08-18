//! Versioned, data-only contracts for universal Agent runtime profiles.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const PROFILE_SCHEMA_VERSION: &str = "omc.profile.v1";
pub const VALIDATION_SCHEMA_VERSION: &str = "omc.profile-validation.v1";
pub const PROBE_SCHEMA_VERSION: &str = "omc.profile-probe.v1";
pub const SETUP_SCHEMA_VERSION: &str = "omc.profile-setup.v1";
pub const DOCTOR_SCHEMA_VERSION: &str = "omc.profile-doctor.v1";

/// Extension names must be namespaced and are preserved across a v1 round trip.
pub type Extensions = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDescriptor {
    pub id: String,
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDescriptor {
    pub id: String,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelDescriptor {
    pub id: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub probeable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProtocolDescriptor {
    McpStdio,
    McpHttpSse {
        endpoint: String,
    },
    ProcessAdapter {
        command: PathBuf,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default = "default_capacity")]
        capacity: usize,
        #[serde(default = "default_idle_ttl_ms")]
        idle_ttl_ms: u64,
    },
}

const fn default_capacity() -> usize {
    4
}

const fn default_idle_ttl_ms() -> u64 {
    60_000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OmcPermission {
    Read,
    WorkspaceWrite,
    ProcessSpawn,
    Network,
    DebugControl,
    CredentialAccess,
    Dangerous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionPolicy {
    #[serde(default)]
    pub omc: Vec<OmcPermission>,
    #[serde(default)]
    pub runtime: Vec<OmcPermission>,
    #[serde(default)]
    pub native_mapping: BTreeMap<String, OmcPermission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    Required,
    Optional,
    Alternative,
    CallerSupplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyDescriptor {
    pub id: String,
    pub kind: DependencyKind,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub version_requirement: Option<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigFormat {
    Json,
    Yaml,
    Toml,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupDescriptor {
    pub format: ConfigFormat,
    pub path: PathBuf,
    pub registration_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub schema_version: String,
    pub id: String,
    pub runtime: RuntimeDescriptor,
    pub provider: ProviderDescriptor,
    pub model: ModelDescriptor,
    pub protocol: ProtocolDescriptor,
    #[serde(default)]
    pub permissions: PermissionPolicy,
    #[serde(default)]
    pub dependencies: Vec<DependencyDescriptor>,
    #[serde(default)]
    pub setup: Option<SetupDescriptor>,
    #[serde(default)]
    pub extensions: Extensions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileSource {
    Explicit,
    Project,
    User,
    Organization,
    BuiltIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub source: ProfileSource,
    pub schema_version: String,
    pub digest: String,
    #[serde(default)]
    pub catalog_version: Option<String>,
    #[serde(default)]
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractIssue {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationReport {
    pub schema_version: String,
    pub valid: bool,
    pub profile: Option<Profile>,
    pub provenance: Option<Provenance>,
    #[serde(default)]
    pub issues: Vec<ContractIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceSource {
    Declared,
    Cataloged,
    Probed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityEvidence {
    pub capability: String,
    pub available: bool,
    pub source: EvidenceSource,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeReport {
    pub schema_version: String,
    pub profile_id: String,
    pub ready: bool,
    pub provenance: Provenance,
    #[serde(default)]
    pub evidence: Vec<CapabilityEvidence>,
    #[serde(default)]
    pub issues: Vec<ContractIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupReport {
    pub schema_version: String,
    pub profile_id: String,
    pub changed: bool,
    pub destination: PathBuf,
    pub provenance: Provenance,
    #[serde(default)]
    pub backup: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoctorReport {
    pub schema_version: String,
    pub profile_id: String,
    pub ready: bool,
    pub provenance: Provenance,
    #[serde(default)]
    pub evidence: Vec<CapabilityEvidence>,
    #[serde(default)]
    pub issues: Vec<ContractIssue>,
    #[serde(default)]
    pub repairs: Vec<RepairGuidance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepairGuidance {
    pub code: String,
    pub summary: String,
    #[serde(default)]
    pub command: Option<String>,
    pub automatic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_identifiers_and_extensions_round_trip() {
        let value = serde_json::json!({
            "schemaVersion": PROFILE_SCHEMA_VERSION,
            "id": "unknown-profile-947",
            "runtime": {"id": "future-runtime", "command": "future-agent"},
            "provider": {"id": "private-provider"},
            "model": {"id": "model-never-seen", "capabilities": ["tool-calling"]},
            "protocol": {"kind": "mcp-stdio"},
            "extensions": {"example.dev/region": "lab"}
        });
        let profile: Profile = serde_json::from_value(value).expect("valid v1 profile");
        assert_eq!(profile.runtime.id, "future-runtime");
        assert_eq!(profile.extensions["example.dev/region"], "lab");
    }

    #[test]
    fn unknown_core_fields_fail_closed() {
        let value = serde_json::json!({
            "id": "runtime", "command": "agent", "surpriseAuthority": true
        });
        assert!(serde_json::from_value::<RuntimeDescriptor>(value).is_err());
    }

    #[test]
    fn released_unknown_runtime_fixture_parses() {
        let fixture = include_str!("../../../schemas/profile-fixtures/unknown-runtime-v1.json");
        let profile: Profile = serde_json::from_str(fixture).expect("released fixture is valid");
        assert_eq!(profile.schema_version, PROFILE_SCHEMA_VERSION);
    }

    #[test]
    fn doctor_repairs_are_explicitly_non_automatic() {
        let repair = RepairGuidance {
            code: "dependency_missing".into(),
            summary: "Install the runtime using its trusted distribution channel".into(),
            command: Some("runtime --version".into()),
            automatic: false,
        };
        let value = serde_json::to_value(repair).unwrap();
        assert_eq!(value["automatic"], false);
    }
}
