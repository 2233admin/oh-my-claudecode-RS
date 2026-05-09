use async_trait::async_trait;

use crate::types::{GitProvider, IssueInfo, PRInfo, PRTerminology, ProviderError, ProviderName};

/// Azure DevOps provider using the `az` CLI.
pub struct AzureDevOpsProvider;

impl Default for AzureDevOpsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AzureDevOpsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GitProvider for AzureDevOpsProvider {
    fn name(&self) -> ProviderName {
        ProviderName::AzureDevOps
    }

    fn display_name(&self) -> &str {
        "Azure DevOps"
    }

    fn pr_terminology(&self) -> PRTerminology {
        PRTerminology::PR
    }

    fn pr_refspec(&self) -> Option<&str> {
        None
    }

    fn detect_from_remote(&self, url: &str) -> bool {
        let lower = url.to_lowercase();
        lower.contains("dev.azure.com")
            || lower.contains("ssh.dev.azure.com")
            || lower.contains("visualstudio.com")
    }

    async fn view_pr(
        &self,
        number: u64,
        _owner: Option<&str>,
        _repo: Option<&str>,
    ) -> Result<PRInfo, ProviderError> {
        if number < 1 {
            return Err(ProviderError::InvalidInput("PR number must be >= 1".into()));
        }

        let output = tokio::process::Command::new("az")
            .args([
                "repos",
                "pr",
                "show",
                "--id",
                &number.to_string(),
                "--output",
                "json",
            ])
            .output()
            .await
            .map_err(|e| ProviderError::ApiError(format!("failed to run az: {e}")))?;

        if !output.status.success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON from az: {e}")))?;

        Ok(PRInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            head_branch: data["sourceRefName"]
                .as_str()
                .map(strip_ref_prefix)
                .map(String::from),
            base_branch: data["targetRefName"]
                .as_str()
                .map(strip_ref_prefix)
                .map(String::from),
            url: data["url"].as_str().map(String::from),
            body: data["description"].as_str().map(String::from),
            author: data["createdBy"]["displayName"].as_str().map(String::from),
        })
    }

    async fn view_issue(
        &self,
        number: u64,
        _owner: Option<&str>,
        _repo: Option<&str>,
    ) -> Result<IssueInfo, ProviderError> {
        if number < 1 {
            return Err(ProviderError::InvalidInput(
                "issue number must be >= 1".into(),
            ));
        }

        let output = tokio::process::Command::new("az")
            .args([
                "boards",
                "work-item",
                "show",
                "--id",
                &number.to_string(),
                "--output",
                "json",
            ])
            .output()
            .await
            .map_err(|e| ProviderError::ApiError(format!("failed to run az: {e}")))?;

        if !output.status.success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON from az: {e}")))?;

        Ok(IssueInfo {
            title: data["fields"]["System.Title"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            body: data["fields"]["System.Description"]
                .as_str()
                .map(String::from),
            labels: Vec::new(),
            url: data["url"].as_str().map(String::from),
        })
    }

    fn check_auth(&self) -> bool {
        std::process::Command::new("az")
            .args(["account", "show"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn required_cli(&self) -> Option<&str> {
        Some("az")
    }
}

fn strip_ref_prefix(ref_name: &str) -> &str {
    ref_name.strip_prefix("refs/heads/").unwrap_or(ref_name)
}
