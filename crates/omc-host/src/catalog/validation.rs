//! Catalog schema, version-range, and same-major compatibility gates.

use super::{CATALOG_SCHEMA_VERSION, Catalog, CatalogError, VersionRange};

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
    for metadata in &catalog.profiles {
        if let Some(profile) = &metadata.profile
            && (profile.id != metadata.id || profile.schema_version != metadata.schema_version)
        {
            return Err(CatalogError::Schema(format!(
                "profile payload does not match metadata for {}",
                metadata.id
            )));
        }
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
