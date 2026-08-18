//! Backward-compatibility gate for released profile contract manifests.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileSchemaManifest {
    pub schema_version: String,
    pub objects: BTreeMap<String, ObjectContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectContract {
    pub fields: BTreeMap<String, FieldContract>,
    #[serde(default)]
    pub required: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldContract {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub values: BTreeSet<String>,
    #[serde(default)]
    pub minimum: Option<i64>,
    #[serde(default)]
    pub maximum: Option<i64>,
}

/// Reject same-major changes that invalidate a client generated from a release.
pub fn verify_profile_compatible(
    baseline_json: &str,
    current: &ProfileSchemaManifest,
) -> Result<(), String> {
    let baseline: ProfileSchemaManifest =
        serde_json::from_str(baseline_json).map_err(|error| error.to_string())?;
    if baseline.schema_version != current.schema_version {
        return Err(format!(
            "schema version changed from {} to {}",
            baseline.schema_version, current.schema_version
        ));
    }
    for (object_name, old_object) in baseline.objects {
        let new_object = current
            .objects
            .get(&object_name)
            .ok_or_else(|| format!("released object removed: {object_name}"))?;
        for (field_name, old_field) in old_object.fields {
            let new_field = new_object
                .fields
                .get(&field_name)
                .ok_or_else(|| format!("{object_name}.{field_name} was removed"))?;
            if old_field.field_type != new_field.field_type {
                return Err(format!("{object_name}.{field_name} type changed"));
            }
            if (!old_field.values.is_empty() && !new_field.values.is_superset(&old_field.values))
                || (old_field.values.is_empty() && !new_field.values.is_empty())
            {
                return Err(format!("{object_name}.{field_name} values were narrowed"));
            }
            if tighter_min(old_field.minimum, new_field.minimum)
                || tighter_max(old_field.maximum, new_field.maximum)
            {
                return Err(format!("{object_name}.{field_name} bounds were tightened"));
            }
        }
        for field_name in &new_object.required {
            if !old_object.required.contains(field_name) {
                return Err(format!("{object_name}.{field_name} became required"));
            }
        }
    }
    Ok(())
}

fn tighter_min(old: Option<i64>, new: Option<i64>) -> bool {
    matches!((old, new), (None, Some(_)))
        || matches!((old, new), (Some(old), Some(new)) if new > old)
}

fn tighter_max(old: Option<i64>, new: Option<i64>) -> bool {
    matches!((old, new), (None, Some(_)))
        || matches!((old, new), (Some(old), Some(new)) if new < old)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn released() -> ProfileSchemaManifest {
        serde_json::from_str(include_str!("../../../schemas/profile-contract-v1.json"))
            .expect("released profile schema fixture")
    }

    #[test]
    fn released_profile_contract_is_self_compatible() {
        let baseline = include_str!("../../../schemas/profile-contract-v1.json");
        assert!(verify_profile_compatible(baseline, &released()).is_ok());
    }

    #[test]
    fn same_major_new_required_field_is_breaking() {
        let baseline = include_str!("../../../schemas/profile-contract-v1.json");
        let mut current = released();
        let profile = current.objects.get_mut("profile").expect("profile object");
        profile.fields.insert(
            "newAuthority".into(),
            FieldContract {
                field_type: "string".into(),
                values: BTreeSet::new(),
                minimum: None,
                maximum: None,
            },
        );
        profile.required.insert("newAuthority".into());
        assert!(verify_profile_compatible(baseline, &current).is_err());
    }

    #[test]
    fn same_major_permission_narrowing_is_breaking() {
        let baseline = include_str!("../../../schemas/profile-contract-v1.json");
        let mut current = released();
        current
            .objects
            .get_mut("permission")
            .expect("permission object")
            .fields
            .get_mut("value")
            .expect("permission value")
            .values
            .remove("network");
        assert!(verify_profile_compatible(baseline, &current).is_err());
    }
}
