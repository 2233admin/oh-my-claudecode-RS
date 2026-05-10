use async_trait::async_trait;

use crate::types::{GitProvider, IssueInfo, PRInfo, PRTerminology, ProviderError, ProviderName};

/// GitLab provider using the `glab` CLI.
#[derive(Default)]
pub struct GitLabProvider;

impl GitLabProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GitProvider for GitLabProvider {
    fn name(&self) -> ProviderName {
        ProviderName::GitLab
    }

    fn display_name(&self) -> &str {
        "GitLab"
    }

    fn pr_terminology(&self) -> PRTerminology {
        PRTerminology::MR
    }

    fn pr_refspec(&self) -> Option<&str> {
        Some("merge-requests/{number}/head:{branch}")
    }

    fn detect_from_remote(&self, url: &str) -> bool {
        let lower = url.to_lowercase();
        if lower.contains("gitlab.com") {
            return true;
        }
        let host = extract_host_from_url(&lower);
        host_label_matches(&host, "gitlab")
    }

    async fn detect_from_api(&self, base_url: &str) -> bool {
        let url = format!("{base_url}/api/v4/version");
        reqwest::Client::new()
            .head(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn view_pr(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<PRInfo, ProviderError> {
        if number < 1 {
            return Err(ProviderError::InvalidInput("MR number must be >= 1".into()));
        }

        let number_str = number.to_string();
        let mut args = vec!["mr", "view", &number_str];
        let repo_arg;
        if let (Some(o), Some(r)) = (owner, repo) {
            repo_arg = format!("{o}/{r}");
            args.push("--repo");
            args.push(&repo_arg);
        }
        args.push("--output");
        args.push("json");

        let output = tokio::process::Command::new("glab")
            .args(&args)
            .output()
            .await
            .map_err(|e| ProviderError::ApiError(format!("failed to run glab: {e}")))?;

        if !output.status.success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON from glab: {e}")))?;

        Ok(PRInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            head_branch: data["source_branch"].as_str().map(String::from),
            base_branch: data["target_branch"].as_str().map(String::from),
            url: data["web_url"].as_str().map(String::from),
            body: data["description"].as_str().map(String::from),
            author: data["author"]["username"].as_str().map(String::from),
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

        let number_str = number.to_string();
        let mut args = vec!["issue", "view", &number_str];
        let repo_arg;
        if let (Some(o), Some(r)) = (owner, repo) {
            repo_arg = format!("{o}/{r}");
            args.push("--repo");
            args.push(&repo_arg);
        }
        args.push("--output");
        args.push("json");

        let output = tokio::process::Command::new("glab")
            .args(&args)
            .output()
            .await
            .map_err(|e| ProviderError::ApiError(format!("failed to run glab: {e}")))?;

        if !output.status.success() {
            return Err(ProviderError::NotFound);
        }

        let data: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| ProviderError::ApiError(format!("invalid JSON from glab: {e}")))?;

        let labels = data["labels"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(IssueInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            body: data["description"].as_str().map(String::from),
            labels,
            url: data["web_url"].as_str().map(String::from),
        })
    }

    fn check_auth(&self) -> bool {
        std::process::Command::new("glab")
            .args(["auth", "status"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    fn required_cli(&self) -> Option<&str> {
        Some("glab")
    }
}

fn extract_host_from_url(url: &str) -> String {
    let s = url.to_lowercase();
    if let Some(pos) = s.find("://") {
        let after = &s[pos + 3..];
        let after_at = after.split_once('@').map_or(after, |(_, h)| h);
        return after_at.split('/').next().unwrap_or("").to_string();
    }
    if let Some(at_pos) = s.find('@') {
        let after = &s[at_pos + 1..];
        return after.split(':').next().unwrap_or("").to_string();
    }
    String::default()
}

fn host_label_matches(host: &str, label: &str) -> bool {
    let prefix_dot = format!("{label}.");
    let prefix_dash = format!("{label}-");
    let dotted = format!(".{label}.");
    let dotted_end = format!(".{label}");
    host.starts_with(&prefix_dot)
        || host.starts_with(&prefix_dash)
        || host.contains(&dotted)
        || host.ends_with(&dotted_end)
        || host == label
}
