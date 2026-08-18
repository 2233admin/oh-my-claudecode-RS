//! Thin CLI surface for trusted catalogs and dependency evidence.

use super::DispatchError;
use crate::commands::{CatalogCommand, DependenciesCommand};
use omc_host::catalog::{CatalogManager, DependencyMetadata};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

const DEPENDENCY_STATUS_SCHEMA: &str = "omc.dependencies-status.v1";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyStatus {
    schema_version: &'static str,
    catalog_version: String,
    dependencies: Vec<DependencyEvidence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyEvidence {
    id: String,
    available: bool,
    command: Option<String>,
    source: &'static str,
}

pub fn run_catalog(command: &CatalogCommand, root: &Path) -> Result<(), DispatchError> {
    let manager = CatalogManager::new(catalog_root(root));
    let status = match command {
        CatalogCommand::Status => manager.active().map(|(_, status)| status),
        CatalogCommand::Refresh {
            source,
            trusted_source,
            trust_key,
        } => manager.refresh_file(source, trusted_source, trust_key),
        CatalogCommand::Rollback => manager.rollback(),
    }
    .map_err(|error| DispatchError::Profile(error.to_string()))?;
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

pub fn run_dependencies(command: &DependenciesCommand, root: &Path) -> Result<(), DispatchError> {
    let manager = CatalogManager::new(catalog_root(root));
    if let DependenciesCommand::Refresh {
        source,
        trusted_source,
        trust_key,
    } = command
    {
        manager
            .refresh_file(source, trusted_source, trust_key)
            .map_err(|error| DispatchError::Profile(error.to_string()))?;
    }
    let (catalog, _) = manager
        .active()
        .map_err(|error| DispatchError::Profile(error.to_string()))?;
    let report = DependencyStatus {
        schema_version: DEPENDENCY_STATUS_SCHEMA,
        catalog_version: catalog.catalog_version,
        dependencies: catalog
            .dependencies
            .iter()
            .map(dependency_evidence)
            .collect(),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn catalog_root(root: &Path) -> PathBuf {
    env::var_os("OMC_CATALOG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".omc/catalogs"))
}

fn dependency_evidence(metadata: &DependencyMetadata) -> DependencyEvidence {
    let command = metadata
        .commands
        .iter()
        .find(|command| command_available(command))
        .cloned();
    DependencyEvidence {
        id: metadata.id.clone(),
        available: command.is_some(),
        command,
        source: "cataloged",
    }
}

fn command_available(command: &str) -> bool {
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|path| {
            let direct = path.join(command);
            direct.is_file() || (cfg!(windows) && path.join(format!("{command}.exe")).is_file())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_evidence_is_catalog_provenance() {
        let evidence = dependency_evidence(&DependencyMetadata {
            id: "definitely-absent-omc-test".into(),
            description: "test".into(),
            commands: vec!["definitely-absent-omc-test".into()],
            platforms: vec![],
        });
        assert_eq!(evidence.source, "cataloged");
        assert!(!evidence.available);
    }
}
