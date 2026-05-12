use async_trait::async_trait;

use crate::types::{GitProvider, IssueInfo, PRInfo, PRTerminology, ProviderError, ProviderName};

const API_BASE: &str = "https://api.bitbucket.org/2.0/repositories";

/// Bitbucket provider using the REST API with bearer or basic auth.
pub struct BitbucketProvider;

impl Default for BitbucketProvider {
    fn default() -> Self {
        Self
    }
}

impl BitbucketProvider {
    pub fn new() -> Self {
        Self
    }

    const BITBUCKET_TOKEN: &str = "BITBUCKET_TOKEN";
    const BITBUCKET_USERNAME: &str = "BITBUCKET_USERNAME";
    const BITBUCKET_APP_PASSWORD: &str = "BITBUCKET_APP_PASSWORD";
    fn auth_header(&self) -> Option<String> {
        if let Ok(token) = std::env::var(Self::BITBUCKET_TOKEN)
            && !token.is_empty()
        {
            return Some(format!("Bearer {token}"));
        }
        if let (Ok(user), Ok(pass)) = (
            std::env::var(Self::BITBUCKET_USERNAME),
            std::env::var(Self::BITBUCKET_APP_PASSWORD),
        ) && !user.is_empty()
            && !pass.is_empty()
        {
            let encoded = base64_encode(&format!("{user}:{pass}"));
            return Some(format!("Basic {encoded}"));
        }
        None
    }
}

#[async_trait]
impl GitProvider for BitbucketProvider {
    fn name(&self) -> ProviderName {
        ProviderName::Bitbucket
    }

    fn display_name(&self) -> &str {
        "Bitbucket"
    }

    fn pr_terminology(&self) -> PRTerminology {
        PRTerminology::PR
    }

    fn pr_refspec(&self) -> Option<&str> {
        None
    }

    fn detect_from_remote(&self, url: &str) -> bool {
        url.to_lowercase().contains("bitbucket.org")
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
        let (o, r) = match (owner, repo) {
            (Some(o), Some(r)) => (o, r),
            _ => {
                return Err(ProviderError::InvalidInput(
                    "owner and repo required".into(),
                ));
            }
        };

        let url = format!("{API_BASE}/{o}/{r}/pullrequests/{number}");
        let auth = self
            .auth_header()
            .ok_or_else(|| ProviderError::AuthFailed("no Bitbucket credentials".into()))?;

        let resp = reqwest::Client::new()
            .get(&url)
            .header("Authorization", &auth)
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
            head_branch: data["source"]["branch"]["name"].as_str().map(String::from),
            base_branch: data["destination"]["branch"]["name"]
                .as_str()
                .map(String::from),
            url: data["links"]["html"]["href"].as_str().map(String::from),
            body: data["description"].as_str().map(String::from),
            author: data["author"]["display_name"].as_str().map(String::from),
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
        let (o, r) = match (owner, repo) {
            (Some(o), Some(r)) => (o, r),
            _ => {
                return Err(ProviderError::InvalidInput(
                    "owner and repo required".into(),
                ));
            }
        };

        let url = format!("{API_BASE}/{o}/{r}/issues/{number}");
        let auth = self
            .auth_header()
            .ok_or_else(|| ProviderError::AuthFailed("no Bitbucket credentials".into()))?;

        let resp = reqwest::Client::new()
            .get(&url)
            .header("Authorization", &auth)
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

        Ok(IssueInfo {
            title: data["title"].as_str().unwrap_or_default().to_string(),
            body: data["content"]["raw"].as_str().map(String::from),
            labels: Vec::new(),
            url: data["links"]["html"]["href"].as_str().map(String::from),
        })
    }

    fn check_auth(&self) -> bool {
        self.auth_header().is_some()
    }

    fn required_cli(&self) -> Option<&str> {
        None
    }
}

/// Minimal base64 encoder (no external dep needed for basic auth).
fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map_or(0, |&b| b as u32);
        let b2 = chunk.get(2).map_or(0, |&b| b as u32);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
