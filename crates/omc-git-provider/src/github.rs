use async_trait::async_trait;

use crate::types::{GitProvider, IssueInfo, PRInfo, PRTerminology, ProviderError, ProviderName};

/// GitHub provider using the `gh` CLI.
pub struct GitHubProvider;

impl Default for GitHubProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GitProvider for GitHubProvider {
    fn name(&self) -> ProviderName {
        ProviderName::GitHub
    }

    fn display_name(&self) -> &str {
        "GitHub"
    }

    fn pr_terminology(&self) -> PRTerminology {
        PRTerminology::PR
    }

    fn pr_refspec(&self) -> Option<&str> {
        Some("pull/{number}/head:{branch}")
    }

    fn detect_from_remote(&self, url: &str) -> bool {
        url.to_lowercase().contains("github.com")
    }

    async fn view_pr(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<PRInfo, ProviderError> {
        if number < 1 {
            return Err(ProviderError::InvalidInput("PR number must be >= 1".into()));
        }

        let num_str = number.to_string();
        let mut args = vec!["pr", "view", num_str.as_str()];
        let repo_arg;
        if let (Some(o), Some(r)) = (owner, repo) {
            repo_arg = format!("{o}/{r}");
            args.push("--repo");
            args.push(&repo_arg);
        }
        args.push("--json");
        args.push("title,headRefName,baseRefName,body,url,author");

        let output = tokio::process::Command::new("gh")
            .args(&args)
            .output()
            .await
            .map_err(|e| ProviderError::ApiError(format!("failed to run gh: {e}")))?;

        if !output.status.success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON from gh: {e}")))?;

        Ok(PRInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            head_branch: data["headRefName"].as_str().map(String::from),
            base_branch: data["baseRefName"].as_str().map(String::from),
            body: data["body"].as_str().map(String::from),
            url: data["url"].as_str().map(String::from),
            author: data["author"]["login"].as_str().map(String::from),
        })
    }

    async fn view_issue(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<IssueInfo, ProviderError> {
        if number < 1 {
            return Err(ProviderError::InvalidInput(
                "issue number must be >= 1".into(),
            ));
        }

        let num_str = number.to_string();
        let mut args = vec!["issue", "view", num_str.as_str()];
        let repo_arg;
        if let (Some(o), Some(r)) = (owner, repo) {
            repo_arg = format!("{o}/{r}");
            args.push("--repo");
            args.push(&repo_arg);
        }
        args.push("--json");
        args.push("title,body,labels,url");

        let output = tokio::process::Command::new("gh")
            .args(&args)
            .output()
            .await
            .map_err(|e| ProviderError::ApiError(format!("failed to run gh: {e}")))?;

        if !output.status.success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON from gh: {e}")))?;

        let labels = data["labels"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(IssueInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            body: data["body"].as_str().map(String::from),
            labels,
            url: data["url"].as_str().map(String::from),
        })
    }

    fn check_auth(&self) -> bool {
        std::process::Command::new("gh")
            .args(["auth", "status"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn required_cli(&self) -> Option<&str> {
        Some("gh")
    }
}
