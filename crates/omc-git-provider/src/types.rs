use async_trait::async_trait;

/// Supported git hosting provider identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderName {
    GitHub,
    GitLab,
    Bitbucket,
    AzureDevOps,
    Gitea,
    Forgejo,
    Unknown,
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::GitLab => write!(f, "gitlab"),
            Self::Bitbucket => write!(f, "bitbucket"),
            Self::AzureDevOps => write!(f, "azure-devops"),
            Self::Gitea => write!(f, "gitea"),
            Self::Forgejo => write!(f, "forgejo"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Parsed remote URL information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUrlInfo {
    pub provider: ProviderName,
    pub host: String,
    pub owner: String,
    pub repo: String,
}

/// Pull request / merge request information.
#[derive(Debug, Clone, Default)]
pub struct PRInfo {
    pub title: String,
    pub head_branch: Option<String>,
    pub base_branch: Option<String>,
    pub url: Option<String>,
    pub body: Option<String>,
    pub author: Option<String>,
}

/// Issue / work item information.
#[derive(Debug, Clone, Default)]
pub struct IssueInfo {
    pub title: String,
    pub body: Option<String>,
    pub labels: Vec<String>,
    pub url: Option<String>,
}

/// PR terminology used by the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PRTerminology {
    /// "Pull Request" (GitHub, Bitbucket, Gitea)
    PR,
    /// "Merge Request" (GitLab)
    MR,
}

impl std::fmt::Display for PRTerminology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PR => write!(f, "PR"),
            Self::MR => write!(f, "MR"),
        }
    }
}

/// Trait implemented by each git hosting provider adapter.
#[async_trait]
pub trait GitProvider: Send + Sync {
    /// Provider identifier.
    fn name(&self) -> ProviderName;

    /// Human-readable name (e.g., "GitHub", "GitLab").
    fn display_name(&self) -> &str;

    /// What this provider calls PRs.
    fn pr_terminology(&self) -> PRTerminology;

    /// Git refspec pattern for fetching PR/MR branches.
    /// Use `{number}` and `{branch}` as placeholders.
    /// `None` if provider doesn't support refspec-based fetching.
    fn pr_refspec(&self) -> Option<&str>;

    /// Check if a remote URL belongs to this provider.
    fn detect_from_remote(&self, url: &str) -> bool;

    /// Probe an API endpoint to detect this provider (for self-hosted).
    async fn detect_from_api(&self, _base_url: &str) -> bool {
        false
    }

    /// Fetch PR/MR information.
    async fn view_pr(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<PRInfo, ProviderError>;

    /// Fetch issue/work-item information.
    async fn view_issue(
        &self,
        number: u64,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> Result<IssueInfo, ProviderError>;

    /// Check if the provider's CLI is authenticated.
    fn check_auth(&self) -> bool;

    /// Return the required CLI tool name, or `None` if API-only.
    fn required_cli(&self) -> Option<&str>;
}

/// Errors from provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("API request failed: {0}")]
    ApiError(String),

    #[error("not found")]
    NotFound,

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
