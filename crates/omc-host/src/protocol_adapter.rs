//! Narrow protocol Adapter seam shared by built-in and unknown profiles.

use async_trait::async_trait;
use omc_shared::profile::{
    CapabilityEvidence, ContractIssue, EvidenceSource, PROBE_SCHEMA_VERSION, ProbeReport,
    ProtocolDescriptor,
};
use serde_json::{Value, json};
use std::time::Duration;
use thiserror::Error;

use crate::profile_lifecycle::{ResolvedProfile, probe_stdio};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol: {0}")]
    Unsupported(String),
    #[error("transport failed: {0}")]
    Transport(String),
}

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    async fn probe(
        &self,
        profile: &ResolvedProfile,
        timeout: Duration,
    ) -> Result<ProbeReport, ProtocolError>;
}

pub struct McpStdioAdapter;

#[async_trait]
impl ProtocolAdapter for McpStdioAdapter {
    async fn probe(
        &self,
        profile: &ResolvedProfile,
        timeout: Duration,
    ) -> Result<ProbeReport, ProtocolError> {
        let profile = profile.clone();
        tokio::task::spawn_blocking(move || probe_stdio(&profile, timeout))
            .await
            .map_err(|error| ProtocolError::Transport(error.to_string()))
    }
}

pub struct McpHttpSseAdapter {
    client: reqwest::Client,
}

impl Default for McpHttpSseAdapter {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ProtocolAdapter for McpHttpSseAdapter {
    async fn probe(
        &self,
        profile: &ResolvedProfile,
        timeout: Duration,
    ) -> Result<ProbeReport, ProtocolError> {
        let ProtocolDescriptor::McpHttpSse { endpoint } = &profile.profile.protocol else {
            return Err(ProtocolError::Unsupported("expected mcp-http-sse".into()));
        };
        let initialize = self
            .request(
                endpoint,
                1,
                "initialize",
                json!({
                    "protocolVersion":"2024-11-05", "capabilities":{},
                    "clientInfo":{"name":"omc-profile-probe","version":"1"}
                }),
                timeout,
            )
            .await;
        let tools = match initialize {
            Ok(_) => {
                self.request(endpoint, 2, "tools/list", json!({}), timeout)
                    .await
            }
            Err(error) => Err(error),
        };
        match tools {
            Ok(value) => {
                let count = value["result"]["tools"].as_array().map_or(0, Vec::len);
                Ok(ProbeReport {
                    schema_version: PROBE_SCHEMA_VERSION.into(),
                    profile_id: profile.profile.id.clone(),
                    ready: true,
                    provenance: profile.provenance.clone(),
                    evidence: vec![CapabilityEvidence {
                        capability: "tool-calling".into(),
                        available: true,
                        source: EvidenceSource::Probed,
                        detail: Some(format!(
                            "HTTP/SSE MCP handshake succeeded; {count} tools discovered"
                        )),
                    }],
                    issues: Vec::new(),
                })
            }
            Err(error) => Ok(ProbeReport {
                schema_version: PROBE_SCHEMA_VERSION.into(),
                profile_id: profile.profile.id.clone(),
                ready: false,
                provenance: profile.provenance.clone(),
                evidence: Vec::new(),
                issues: vec![ContractIssue {
                    code: "http_sse_probe_failed".into(),
                    path: "$.protocol.endpoint".into(),
                    message: error.to_string(),
                }],
            }),
        }
    }
}

impl McpHttpSseAdapter {
    async fn request(
        &self,
        endpoint: &str,
        id: u64,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, ProtocolError> {
        let response = self
            .client
            .post(endpoint)
            .header("accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .timeout(timeout)
            .send()
            .await
            .map_err(|error| ProtocolError::Transport(error.to_string()))?
            .error_for_status()
            .map_err(|error| ProtocolError::Transport(error.to_string()))?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = response
            .text()
            .await
            .map_err(|error| ProtocolError::Transport(error.to_string()))?;
        let payload = if content_type.contains("text/event-stream") {
            body.lines()
                .find_map(|line| line.strip_prefix("data:"))
                .map(str::trim)
                .ok_or_else(|| ProtocolError::Transport("SSE response has no data event".into()))?
        } else {
            body.trim()
        };
        serde_json::from_str(payload).map_err(|error| ProtocolError::Transport(error.to_string()))
    }
}

pub fn adapter_for(
    protocol: &ProtocolDescriptor,
) -> Result<Box<dyn ProtocolAdapter>, ProtocolError> {
    match protocol {
        ProtocolDescriptor::McpStdio => Ok(Box::new(McpStdioAdapter)),
        ProtocolDescriptor::McpHttpSse { .. } => Ok(Box::new(McpHttpSseAdapter::default())),
        ProtocolDescriptor::ProcessAdapter { .. } => Err(ProtocolError::Unsupported(
            "process adapter requires explicit authorization".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_lifecycle::ResolvedProfile;
    use omc_shared::profile::{
        ModelDescriptor, PermissionPolicy, Profile, ProfileSource, Provenance, ProviderDescriptor,
        RuntimeDescriptor,
    };
    use std::collections::BTreeMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn process_adapter_is_not_implicitly_authorized() {
        let protocol = ProtocolDescriptor::ProcessAdapter {
            command: "adapter".into(),
            args: Vec::new(),
            capacity: 1,
            idle_ttl_ms: 10,
        };
        assert!(adapter_for(&protocol).is_err());
    }

    fn resolved(
        protocol: ProtocolDescriptor,
        command: String,
        args: Vec<String>,
    ) -> ResolvedProfile {
        ResolvedProfile {
            profile: Profile {
                schema_version: "omc.profile.v1".into(),
                id: "equivalent-runtime".into(),
                runtime: RuntimeDescriptor {
                    id: "equivalent-runtime".into(),
                    command: command.into(),
                    args,
                    environment: BTreeMap::new(),
                },
                provider: ProviderDescriptor {
                    id: "p".into(),
                    endpoint: None,
                },
                model: ModelDescriptor {
                    id: "m".into(),
                    capabilities: vec!["tool-calling".into()],
                    probeable: true,
                },
                protocol,
                permissions: PermissionPolicy::default(),
                dependencies: Vec::new(),
                setup: None,
                extensions: BTreeMap::new(),
            },
            provenance: Provenance {
                source: ProfileSource::Explicit,
                schema_version: "omc.profile.v1".into(),
                digest: "a".repeat(64),
                catalog_version: None,
                location: None,
            },
        }
    }

    #[tokio::test]
    async fn stdio_and_http_sse_discover_equivalent_tool_capabilities() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for response in [
                json!({"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}),
                json!({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo"}]}}),
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let _ = socket.read(&mut request).await.unwrap();
                let body = response.to_string();
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });

        #[cfg(windows)]
        let (command, args) = (
            "powershell".to_string(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                "Write-Output '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{}}}'; Write-Output '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"echo\"}]}}'".into(),
            ],
        );
        #[cfg(not(windows))]
        let (command, args) = (
            "sh".to_string(),
            vec![
                "-c".into(),
                "printf '%s\\n%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{}}}' '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"echo\"}]}}'".into(),
            ],
        );
        let stdio = resolved(ProtocolDescriptor::McpStdio, command, args);
        let http = resolved(
            ProtocolDescriptor::McpHttpSse {
                endpoint: format!("http://{address}/mcp"),
            },
            "unused".into(),
            Vec::new(),
        );

        let stdio_report = adapter_for(&stdio.profile.protocol)
            .unwrap()
            .probe(&stdio, Duration::from_secs(2))
            .await
            .unwrap();
        let http_report = adapter_for(&http.profile.protocol)
            .unwrap()
            .probe(&http, Duration::from_secs(2))
            .await
            .unwrap();
        server.await.unwrap();

        assert!(stdio_report.ready);
        assert!(http_report.ready);
        assert_eq!(
            stdio_report.evidence[0].capability,
            http_report.evidence[0].capability
        );
        assert_eq!(
            stdio_report.evidence[0].available,
            http_report.evidence[0].available
        );
        assert!(
            stdio_report.evidence[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("1 tools")
        );
        assert!(
            http_report.evidence[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("1 tools")
        );
    }
}
