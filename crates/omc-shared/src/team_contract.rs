//! Stable read-only projection of the existing OMC team observability state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TEAM_OBSERVABILITY_SCHEMA_VERSION: &str = "omc.team-observability.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamObservabilityPayload {
    pub schema_version: String,
    pub operation: String,
    pub view: String,
    pub data: Value,
}

impl TeamObservabilityPayload {
    pub fn new(view: impl Into<String>, data: Value) -> Self {
        Self {
            schema_version: TEAM_OBSERVABILITY_SCHEMA_VERSION.into(),
            operation: "team.observability".into(),
            view: view.into(),
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_has_stable_nested_schema() {
        let value = serde_json::to_value(TeamObservabilityPayload::new(
            "sessions",
            serde_json::json!([]),
        ))
        .unwrap();
        assert_eq!(value["schemaVersion"], TEAM_OBSERVABILITY_SCHEMA_VERSION);
        assert_eq!(value["operation"], "team.observability");
        assert_eq!(value["view"], "sessions");
    }
}
