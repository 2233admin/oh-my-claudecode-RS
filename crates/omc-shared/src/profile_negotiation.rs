//! Fail-closed capability and permission negotiation for runtime profiles.

use crate::profile::{CapabilityEvidence, EvidenceSource, OmcPermission};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveCapability {
    pub capability: String,
    pub available: bool,
    pub evidence: Vec<CapabilityEvidence>,
    pub conflict: bool,
}

/// A negative catalog or probe observation overrides declarations.
pub fn negotiate_capabilities(evidence: &[CapabilityEvidence]) -> Vec<EffectiveCapability> {
    let mut grouped: BTreeMap<String, Vec<CapabilityEvidence>> = BTreeMap::new();
    for item in evidence {
        grouped
            .entry(item.capability.clone())
            .or_default()
            .push(item.clone());
    }
    grouped
        .into_iter()
        .map(|(capability, evidence)| {
            let has_positive = evidence.iter().any(|item| item.available);
            let authoritative_negative = evidence.iter().any(|item| {
                !item.available
                    && matches!(
                        item.source,
                        EvidenceSource::Cataloged | EvidenceSource::Probed
                    )
            });
            let has_probe = evidence
                .iter()
                .any(|item| item.source == EvidenceSource::Probed);
            let available = has_positive && !authoritative_negative;
            let conflict = evidence.iter().any(|item| item.available != available)
                || (has_probe
                    && evidence
                        .iter()
                        .filter(|item| item.source == EvidenceSource::Probed)
                        .any(|item| !item.available));
            EffectiveCapability {
                capability,
                available,
                evidence,
                conflict,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    pub effective: BTreeSet<OmcPermission>,
    pub denied: BTreeSet<OmcPermission>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PermissionError {
    #[error("unknown or unmapped native permission: {0}")]
    UnknownNative(String),
    #[error("lower-trust override attempted to expand permission: {0:?}")]
    LowerTrustExpansion(OmcPermission),
}

pub fn map_native_permissions(
    native: impl IntoIterator<Item = String>,
    mapping: &BTreeMap<String, OmcPermission>,
) -> Result<BTreeSet<OmcPermission>, PermissionError> {
    native
        .into_iter()
        .map(|permit| {
            mapping
                .get(&permit)
                .copied()
                .ok_or(PermissionError::UnknownNative(permit))
        })
        .collect()
}

pub fn intersect_permissions(
    omc: &BTreeSet<OmcPermission>,
    runtime: &BTreeSet<OmcPermission>,
) -> PermissionDecision {
    let effective: BTreeSet<OmcPermission> = omc.intersection(runtime).copied().collect();
    let denied = omc
        .union(runtime)
        .copied()
        .filter(|item| !effective.contains(item))
        .collect();
    PermissionDecision { effective, denied }
}

pub fn ensure_override_only_tightens(
    trusted: &BTreeSet<OmcPermission>,
    lower_trust: &BTreeSet<OmcPermission>,
) -> Result<(), PermissionError> {
    lower_trust
        .difference(trusted)
        .next()
        .copied()
        .map_or(Ok(()), |permission| {
            Err(PermissionError::LowerTrustExpansion(permission))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FirstUseVerification {
    NotRequired,
    Required,
    Verified,
    Failed,
}

pub fn first_use_state(probeable: bool, dangerous: bool) -> FirstUseVerification {
    if !probeable && dangerous {
        FirstUseVerification::Required
    } else {
        FirstUseVerification::NotRequired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(source: EvidenceSource, available: bool) -> CapabilityEvidence {
        CapabilityEvidence {
            capability: "tool-calling".into(),
            available,
            source,
            detail: None,
        }
    }

    #[test]
    fn probe_denial_overrides_declared_capability() {
        let result = negotiate_capabilities(&[
            evidence(EvidenceSource::Declared, true),
            evidence(EvidenceSource::Probed, false),
        ]);
        assert!(!result[0].available);
        assert!(result[0].conflict);
    }

    #[test]
    fn unknown_native_permission_fails_closed() {
        assert_eq!(
            map_native_permissions(["root-everything".into()], &BTreeMap::new()),
            Err(PermissionError::UnknownNative("root-everything".into()))
        );
    }

    #[test]
    fn either_policy_can_deny() {
        let omc = [OmcPermission::Read, OmcPermission::ProcessSpawn].into();
        let runtime = [OmcPermission::Read].into();
        let decision = intersect_permissions(&omc, &runtime);
        assert_eq!(decision.effective, [OmcPermission::Read].into());
        assert!(decision.denied.contains(&OmcPermission::ProcessSpawn));
    }

    #[test]
    fn lower_trust_cannot_expand_and_unprobeable_dangerous_requires_verification() {
        let trusted = [OmcPermission::Read].into();
        let expanded = [OmcPermission::Read, OmcPermission::Network].into();
        assert!(ensure_override_only_tightens(&trusted, &expanded).is_err());
        assert_eq!(first_use_state(false, true), FirstUseVerification::Required);
    }
}
