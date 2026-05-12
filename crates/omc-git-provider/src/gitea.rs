use async_trait::async_trait;

use crate::types::{GitProvider, IssueInfo, PRInfo, PRTerminology, ProviderError, ProviderName};

/// Gitea / Forgejo provider.
///
/// Tries the `tea` CLI first, falls back to REST API via `GITEA_URL` + `GITEA_TOKEN`.
pub struct GiteaProvider {
    variant: ProviderName,
    display_name: String,
}

impl Default for GiteaProvider {
    fn default() -> Self {
        Self {
            variant: ProviderName::Gitea,
            display_name: "Gitea".to_string(),
        }
    }
}

impl GiteaProvider {
    pub fn new() -> Self {
        Self {
            variant: ProviderName::Gitea,
            display_name: "Gitea".to_string(),
        }
    }

    pub fn forgejo() -> Self {
        Self {
            variant: ProviderName::Forgejo,
            display_name: "Forgejo".to_string(),
        }
    }

    fn api_base(&self) -> Option<String> {
        std::env::var("GITEA_URL").ok().filter(|u| !u.is_empty())
    }

    fn api_token(&self) -> Option<String> {
        std::env::var("GITEA_TOKEN").ok().filter(|t| !t.is_empty())
    }
}

#[async_trait]
impl GitProvider for GiteaProvider {
    fn name(&self) -> ProviderName {
        self.variant
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn pr_terminology(&self) -> PRTerminology {
        PRTerminology::PR
    }

    fn pr_refspec(&self) -> Option<&str> {
        None
    }

    fn detect_from_remote(&self, _url: &str) -> bool {
        // Self-hosted: can't reliably detect from URL alone
        false
    }

    async fn detect_from_api(&self, base_url: &str) -> bool {
        // Try Forgejo endpoint first
        let forgejo_url = format!("{base_url}/api/forgejo/v1/version");
        if let Ok(resp) = reqwest::Client::new().head(&forgejo_url).send().await
            && resp.status().is_success()
        {
            return true;
        }
        // Fall back to Gitea endpoint
        let gitea_url = format!("{base_url}/api/v1/version");
        reqwest::Client::new()
            .head(&gitea_url)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
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

        // Try tea CLI first
        if let Ok(output) = tokio::process::Command::new("tea")
            .args(["pr", "view", &number.to_string()])
            .output()
            .await
            && output.status.success()
            && let Ok(data) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        {
            return Ok(PRInfo {
                title: data["title"].as_str().unwrap_or_default().to_string(),
                head_branch: data["head_branch"].as_str().map(String::from),
                base_branch: data["base_branch"].as_str().map(String::from),
                url: data["html_url"].as_str().map(String::from),
                body: data["body"].as_str().map(String::from),
                author: data["user"]["login"].as_str().map(String::from),
            });
        }

        // Fall back to REST API
        self.view_pr_rest(number, owner, repo).await
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

        // Try tea CLI first
        if let Ok(output) = tokio::process::Command::new("tea")
            .args(["issues", "view", &number.to_string()])
            .output()
            .await
            && output.status.success()
            && let Ok(data) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        {
            let labels = data["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(IssueInfo {
                title: data["title"].as_str().unwrap_or_default().to_string(),
                body: data["body"].as_str().map(String::from),
                labels,
                url: data["html_url"].as_str().map(String::from),
            });
        }

        // Fall back to REST API
        self.view_issue_rest(number, owner, repo).await
    }

    fn check_auth(&self) -> bool {
        if self.api_token().is_some() {
            return true;
        }
        std::process::Command::new("tea")
            .args(["login", "list"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn required_cli(&self) -> Option<&str> {
        None
    }
}

impl GiteaProvider {
    async fn view_pr_rest(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<PRInfo, ProviderError> {
        let base = self
            .api_base()
            .ok_or_else(|| ProviderError::AuthFailed("GITEA_URL not set".into()))?;
        let (o, r) = match (owner, repo) {
            (Some(o), Some(r)) => (o, r),
            _ => {
                return Err(ProviderError::InvalidInput(
                    "owner and repo required".into(),
                ));
            }
        };

        let url = format!("{base}/api/v1/repos/{o}/{r}/pulls/{number}");
        let client = reqwest::Client::new();
        let mut req = client.get(&url);
        if let Some(token) = self.api_token() {
            req = req.bearer_auth(token);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ProviderError::ApiError(format!("request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON: {e}")))?;

        Ok(PRInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            head_branch: data["head"]["ref"]
                .as_str()
                .or_else(|| data["head_branch"].as_str())
                .map(String::from),
            base_branch: data["base"]["ref"]
                .as_str()
                .or_else(|| data["base_branch"].as_str())
                .map(String::from),
            url: data["html_url"].as_str().map(String::from),
            body: data["body"].as_str().map(String::from),
            author: data["user"]["login"].as_str().map(String::from),
        })
    }

    async fn view_issue_rest(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<IssueInfo, ProviderError> {
        let base = self
            .api_base()
            .ok_or_else(|| ProviderError::AuthFailed("GITEA_URL not set".into()))?;
        let (o, r) = match (owner, repo) {
            (Some(o), Some(r)) => (o, r),
            _ => {
                return Err(ProviderError::InvalidInput(
                    "owner and repo required".into(),
                ));
            }
        };

        let url = format!("{base}/api/v1/repos/{o}/{r}/issues/{number}");
        let client = reqwest::Client::new();
        let mut req = client.get(&url);
        if let Some(token) = self.api_token() {
            req = req.bearer_auth(token);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ProviderError::ApiError(format!("request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON: {e}")))?;

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
            url: data["html_url"].as_str().map(String::from),
        })
    }
}
