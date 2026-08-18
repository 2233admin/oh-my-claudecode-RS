//! Versioned compatibility checks for the public MCP tool surface.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::McpToolRegistry;
use crate::tools::{SchemaProperty, ToolSchema};

/// Stable, machine-readable MCP schema manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaManifest {
    pub schema_version: String,
    pub tools: Vec<SchemaTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaTool {
    pub name: String,
    pub input_schema: StableToolSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: BTreeMap<String, StableProperty>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableProperty {
    #[serde(rename = "type")]
    pub property_type: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub r#enum: BTreeSet<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
}

impl From<ToolSchema> for StableToolSchema {
    fn from(schema: ToolSchema) -> Self {
        Self {
            schema_type: schema.schema_type,
            properties: schema
                .properties
                .into_iter()
                .map(|(name, property)| (name, property.into()))
                .collect(),
            required: schema.required.into_iter().collect(),
        }
    }
}

impl From<SchemaProperty> for StableProperty {
    fn from(property: SchemaProperty) -> Self {
        Self {
            property_type: property.prop_type,
            r#enum: property.r#enum.unwrap_or_default().into_iter().collect(),
            max_length: property.max_length,
            minimum: property.minimum,
            maximum: property.maximum,
        }
    }
}

/// Snapshot the effective production registry, excluding mutable descriptions.
pub fn current_manifest() -> SchemaManifest {
    let mut tools = McpToolRegistry::all_enabled()
        .into_tools()
        .into_iter()
        .map(|tool| {
            let definition = tool.definition();
            SchemaTool {
                name: definition.name,
                input_schema: definition.input_schema.into(),
            }
        })
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    SchemaManifest {
        schema_version: "omc.mcp-tools.v1".to_string(),
        tools,
    }
}

/// Reject changes that would invalidate a client generated from `baseline_json`.
pub fn verify_backward_compatible(
    baseline_json: &str,
    current: &SchemaManifest,
) -> Result<(), String> {
    let baseline: SchemaManifest =
        serde_json::from_str(baseline_json).map_err(|error| error.to_string())?;
    if baseline.schema_version != current.schema_version {
        return Err(format!(
            "schema version changed from {} to {}",
            baseline.schema_version, current.schema_version
        ));
    }

    let current_tools = current
        .tools
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    for released in &baseline.tools {
        let Some(candidate) = current_tools.get(released.name.as_str()) else {
            return Err(format!("released tool removed: {}", released.name));
        };
        compare_schema(released, candidate)?;
    }
    Ok(())
}

fn compare_schema(released: &SchemaTool, candidate: &SchemaTool) -> Result<(), String> {
    if released.input_schema.schema_type != candidate.input_schema.schema_type {
        return Err(format!("{} input type changed", released.name));
    }
    for (name, old) in &released.input_schema.properties {
        let Some(new) = candidate.input_schema.properties.get(name) else {
            return Err(format!("{}.{} was removed", released.name, name));
        };
        if old.property_type != new.property_type {
            return Err(format!("{}.{} type changed", released.name, name));
        }
        if (old.r#enum.is_empty() && !new.r#enum.is_empty())
            || (!old.r#enum.is_empty() && !new.r#enum.is_superset(&old.r#enum))
        {
            return Err(format!("{}.{} enum was narrowed", released.name, name));
        }
        if tighter_max(old.max_length, new.max_length)
            || tighter_max_i64(old.maximum, new.maximum)
            || tighter_min(old.minimum, new.minimum)
        {
            return Err(format!(
                "{}.{} constraint was tightened",
                released.name, name
            ));
        }
    }
    for required in &candidate.input_schema.required {
        if !released.input_schema.required.contains(required) {
            return Err(format!("{}.{} became required", released.name, required));
        }
    }
    Ok(())
}

fn tighter_max(old: Option<u64>, new: Option<u64>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => new < old,
        _ => false,
    }
}

fn tighter_max_i64(old: Option<i64>, new: Option<i64>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => new < old,
        _ => false,
    }
}

fn tighter_min(old: Option<i64>, new: Option<i64>) -> bool {
    match (old, new) {
        (None, Some(_)) => true,
        (Some(old), Some(new)) => new > old,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_an_optional_property_is_compatible() {
        let baseline = manifest_with_property(false);
        let mut current = baseline.clone();
        current.tools[0].input_schema.properties.insert(
            "extra".to_string(),
            StableProperty {
                property_type: "string".to_string(),
                r#enum: BTreeSet::new(),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        let json = serde_json::to_string(&baseline).unwrap();
        assert!(verify_backward_compatible(&json, &current).is_ok());
    }

    #[test]
    fn making_a_property_required_is_breaking() {
        let baseline = manifest_with_property(false);
        let current = manifest_with_property(true);
        let json = serde_json::to_string(&baseline).unwrap();
        assert!(verify_backward_compatible(&json, &current).is_err());
    }

    #[test]
    fn introducing_an_enum_is_breaking() {
        let baseline = manifest_with_property(false);
        let mut current = baseline.clone();
        current.tools[0]
            .input_schema
            .properties
            .get_mut("value")
            .unwrap()
            .r#enum = ["only".to_string()].into_iter().collect();
        let json = serde_json::to_string(&baseline).unwrap();
        assert!(verify_backward_compatible(&json, &current).is_err());
    }

    fn manifest_with_property(required: bool) -> SchemaManifest {
        let property = StableProperty {
            property_type: "string".to_string(),
            r#enum: BTreeSet::new(),
            max_length: None,
            minimum: None,
            maximum: None,
        };
        SchemaManifest {
            schema_version: "omc.mcp-tools.v1".to_string(),
            tools: vec![SchemaTool {
                name: "example".to_string(),
                input_schema: StableToolSchema {
                    schema_type: "object".to_string(),
                    properties: [("value".to_string(), property)].into_iter().collect(),
                    required: required.then(|| "value".to_string()).into_iter().collect(),
                },
            }],
        }
    }
}
