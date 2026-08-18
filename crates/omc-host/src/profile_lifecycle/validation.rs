//! Semantic validation for resolved universal profiles.

use super::ResolvedProfile;
use omc_shared::profile::{
    ContractIssue, PROFILE_SCHEMA_VERSION, ProtocolDescriptor, VALIDATION_SCHEMA_VERSION,
    ValidationReport,
};

pub fn validate_profile(resolved: ResolvedProfile) -> ValidationReport {
    let mut issues = Vec::new();
    if resolved.profile.schema_version != PROFILE_SCHEMA_VERSION {
        issues.push(issue(
            "migration_required",
            "$.schemaVersion",
            format!(
                "unsupported profile schema {}; expected {PROFILE_SCHEMA_VERSION}",
                resolved.profile.schema_version
            ),
        ));
    }
    validate_identifier("$.id", &resolved.profile.id, &mut issues);
    validate_identifier("$.runtime.id", &resolved.profile.runtime.id, &mut issues);
    validate_identifier("$.provider.id", &resolved.profile.provider.id, &mut issues);
    validate_identifier("$.model.id", &resolved.profile.model.id, &mut issues);
    if resolved.profile.runtime.command.as_os_str().is_empty() {
        issues.push(issue("required", "$.runtime.command", "command is empty"));
    }
    for (name, value) in &resolved.profile.runtime.environment {
        if looks_secret(name) || looks_secret(value) {
            issues.push(issue(
                "secret_value_forbidden",
                format!("$.runtime.environment.{name}"),
                "profiles may reference secret names but may not contain secret values",
            ));
        }
    }
    if let ProtocolDescriptor::McpHttpSse { endpoint } = &resolved.profile.protocol {
        let valid = reqwest::Url::parse(endpoint).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        });
        if !valid {
            issues.push(issue(
                "invalid_endpoint",
                "$.protocol.endpoint",
                "HTTP/SSE endpoint must be an absolute http(s) URL without credentials",
            ));
        }
    }
    for key in resolved.profile.extensions.keys() {
        if !key.contains('/') {
            issues.push(issue(
                "extension_not_namespaced",
                format!("$.extensions.{key}"),
                "extension keys must contain an owner namespace",
            ));
        }
    }
    ValidationReport {
        schema_version: VALIDATION_SCHEMA_VERSION.into(),
        valid: issues.is_empty(),
        profile: issues.is_empty().then_some(resolved.profile),
        provenance: Some(resolved.provenance),
        issues,
    }
}

fn validate_identifier(path: &str, value: &str, issues: &mut Vec<ContractIssue>) {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._/:".contains(character))
    {
        issues.push(issue(
            "invalid_identifier",
            path,
            "identifier must be a non-empty portable open string",
        ));
    }
}

fn looks_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("secret")
        || lower.contains("token=")
        || lower.starts_with("sk-")
}

fn issue(code: &str, path: impl Into<String>, message: impl Into<String>) -> ContractIssue {
    ContractIssue {
        code: code.into(),
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_detection_covers_key_names_and_values() {
        assert!(looks_secret("API_KEY"));
        assert!(looks_secret("sk-private"));
        assert!(!looks_secret("MODEL_NAME"));
    }
}
