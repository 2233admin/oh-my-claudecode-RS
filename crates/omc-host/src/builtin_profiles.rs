//! Bundled compatibility profiles for established MCP consumers.

use omc_shared::profile::{
    ConfigFormat, ModelDescriptor, PROFILE_SCHEMA_VERSION, PermissionPolicy, Profile,
    ProtocolDescriptor, ProviderDescriptor, RuntimeDescriptor, SetupDescriptor,
};
use std::collections::BTreeMap;
use std::path::Path;

pub const GENERIC_MCP_PROFILE: &str = "generic-mcp";
pub const HERMES_PROFILE: &str = "hermes";
pub const CLAUDE_PROFILE: &str = "claude";
pub const CODEX_PROFILE: &str = "codex";

pub fn bundled_profiles(hermes_home: &Path) -> BTreeMap<String, Profile> {
    [
        (GENERIC_MCP_PROFILE, profile(GENERIC_MCP_PROFILE, None)),
        (
            HERMES_PROFILE,
            profile(
                HERMES_PROFILE,
                Some(SetupDescriptor {
                    format: ConfigFormat::Yaml,
                    path: hermes_home.join("config.yaml"),
                    registration_path: vec!["mcp_servers".into(), "omc-rs".into()],
                }),
            ),
        ),
        (
            CLAUDE_PROFILE,
            profile(
                CLAUDE_PROFILE,
                Some(SetupDescriptor {
                    format: ConfigFormat::Json,
                    path: ".mcp.json".into(),
                    registration_path: vec!["mcpServers".into(), "omc-rs".into()],
                }),
            ),
        ),
        (
            CODEX_PROFILE,
            profile(
                CODEX_PROFILE,
                Some(SetupDescriptor {
                    format: ConfigFormat::Toml,
                    path: ".codex/config.toml".into(),
                    registration_path: vec!["mcp_servers".into(), "omc-rs".into()],
                }),
            ),
        ),
    ]
    .into_iter()
    .map(|(id, profile)| (id.to_string(), profile))
    .collect()
}

fn profile(id: &str, setup: Option<SetupDescriptor>) -> Profile {
    Profile {
        schema_version: PROFILE_SCHEMA_VERSION.into(),
        id: id.into(),
        runtime: RuntimeDescriptor {
            id: id.into(),
            command: "omc".into(),
            args: vec!["mcp".into()],
            environment: BTreeMap::new(),
        },
        provider: ProviderDescriptor {
            id: "runtime-managed".into(),
            endpoint: None,
        },
        model: ModelDescriptor {
            id: "runtime-managed".into(),
            capabilities: vec!["tool-calling".into()],
            probeable: false,
        },
        protocol: ProtocolDescriptor::McpStdio,
        permissions: PermissionPolicy::default(),
        dependencies: Vec::new(),
        setup,
        extensions: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_lifecycle::{
        ProfileRef, ResolutionContext, resolve_profile, setup_profile, validate_profile,
    };

    #[test]
    fn all_compatibility_profiles_share_the_open_contract() {
        let profiles = bundled_profiles(Path::new("/tmp/hermes"));
        assert_eq!(profiles.len(), 4);
        assert!(
            profiles
                .values()
                .all(|profile| profile.schema_version == PROFILE_SCHEMA_VERSION)
        );
        assert!(
            profiles
                .values()
                .all(|profile| matches!(profile.protocol, ProtocolDescriptor::McpStdio))
        );
    }

    #[test]
    fn every_builtin_passes_the_common_lifecycle_conformance_harness() {
        let root = tempfile::tempdir().unwrap();
        let hermes_home = root.path().join("hermes");
        let profiles = bundled_profiles(&hermes_home);

        for id in [
            GENERIC_MCP_PROFILE,
            HERMES_PROFILE,
            CLAUDE_PROFILE,
            CODEX_PROFILE,
        ] {
            let resolved = resolve_profile(
                &ProfileRef::Named(id.into()),
                &ResolutionContext {
                    project_root: root.path().into(),
                    user_home: root.path().join("user"),
                    organization_catalog: None,
                    built_ins: profiles.clone(),
                },
            )
            .unwrap();
            let validation = validate_profile(resolved.clone());
            assert!(validation.valid, "{id}: {:?}", validation.issues);

            if resolved.profile.setup.is_some() {
                let first = setup_profile(&resolved, root.path(), Default::default()).unwrap();
                let second = setup_profile(&resolved, root.path(), Default::default()).unwrap();
                assert!(first.changed, "{id} did not install through common setup");
                assert!(!second.changed, "{id} common setup is not idempotent");
            }
        }
    }
}
