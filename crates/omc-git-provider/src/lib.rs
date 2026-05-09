pub mod azure_devops;
pub mod bitbucket;
pub mod gitea;
pub mod github;
pub mod gitlab;

mod types;

pub use types::{GitProvider, IssueInfo, PRInfo, ProviderName, RemoteUrlInfo};

use std::collections::HashMap;
use std::sync::OnceLock;

/// Provider registry keyed by ProviderName.
type Registry = HashMap<ProviderName, Box<dyn GitProvider>>;

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn init_registry() -> Registry {
    let mut map: Registry = HashMap::new();
    map.insert(
        ProviderName::GitHub,
        Box::new(github::GitHubProvider::new()),
    );
    map.insert(
        ProviderName::GitLab,
        Box::new(gitlab::GitLabProvider::new()),
    );
    map.insert(
        ProviderName::Bitbucket,
        Box::new(bitbucket::BitbucketProvider::new()),
    );
    map.insert(
        ProviderName::AzureDevOps,
        Box::new(azure_devops::AzureDevOpsProvider::new()),
    );
    map.insert(ProviderName::Gitea, Box::new(gitea::GiteaProvider::new()));
    map.insert(
        ProviderName::Forgejo,
        Box::new(gitea::GiteaProvider::forgejo()),
    );
    map
}

/// Get a provider instance by name.
pub fn get_provider(name: &ProviderName) -> Option<&'static dyn GitProvider> {
    let registry = REGISTRY.get_or_init(init_registry);
    registry.get(name).map(|p| p.as_ref())
}

/// Detect the provider from a git remote URL.
pub fn detect_provider(remote_url: &str) -> ProviderName {
    let url = remote_url.to_lowercase();
    let host = extract_host(&url);
    let host = host.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');

    // Azure DevOps (check before generic patterns)
    if host.contains("dev.azure.com")
        || host.contains("ssh.dev.azure.com")
        || host.ends_with(".visualstudio.com")
    {
        return ProviderName::AzureDevOps;
    }

    if host == "github.com" {
        return ProviderName::GitHub;
    }
    if host == "gitlab.com" {
        return ProviderName::GitLab;
    }
    if host == "bitbucket.org" {
        return ProviderName::Bitbucket;
    }

    // Self-hosted heuristics
    if contains_host_label(host, "gitlab") {
        return ProviderName::GitLab;
    }
    if contains_host_label(host, "gitea") {
        return ProviderName::Gitea;
    }
    if contains_host_label(host, "forgejo") {
        return ProviderName::Forgejo;
    }

    ProviderName::Unknown
}

/// Parse a git remote URL into structured components.
pub fn parse_remote_url(url: &str) -> Option<RemoteUrlInfo> {
    let trimmed = url.trim();
    if let Some(info) = try_parse_azure_https(trimmed) {
        return Some(info);
    }
    if let Some(info) = try_parse_azure_ssh(trimmed) {
        return Some(info);
    }
    if let Some(info) = try_parse_azure_legacy(trimmed) {
        return Some(info);
    }
    if let Some(info) = try_parse_https(trimmed) {
        return Some(info);
    }
    if let Some(info) = try_parse_ssh_url(trimmed) {
        return Some(info);
    }
    if let Some(info) = try_parse_ssh_scp(trimmed) {
        return Some(info);
    }
    None
}

// ---------------------------------------------------------------------------
// URL parsing helpers
// ---------------------------------------------------------------------------

fn extract_host(url: &str) -> &str {
    let s = url.to_lowercase();
    if let Some(pos) = s.find("://") {
        let after_scheme = &url[pos + 3..];
        let after_at = after_scheme
            .split_once('@')
            .map(|(_, h)| h)
            .unwrap_or(after_scheme);
        return after_at.split('/').next().unwrap_or("");
    }
    if let Some(at_pos) = s.find('@') {
        let after_at = &url[at_pos + 1..];
        return after_at.split(':').next().unwrap_or("");
    }
    ""
}

fn contains_host_label(host: &str, label: &str) -> bool {
    // Match label as a hostname segment: preceded/followed by . or -
    let prefix_dot = format!("{label}.");
    let prefix_dash = format!("{label}-");
    let dotted_mid = format!(".{label}.");
    let dotted_end = format!(".{label}");
    host.starts_with(&prefix_dot)
        || host.starts_with(&prefix_dash)
        || host.contains(&dotted_mid)
        || host.ends_with(&dotted_end)
        || host == label
}

fn try_parse_azure_https(url: &str) -> Option<RemoteUrlInfo> {
    let prefix = "https://dev.azure.com/";
    let rest = url
        .strip_prefix(prefix)
        .or_else(|| url.strip_prefix("http://dev.azure.com/"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() >= 4 && parts[2] == "_git" {
        Some(RemoteUrlInfo {
            provider: ProviderName::AzureDevOps,
            host: "dev.azure.com".to_string(),
            owner: format!("{}/{}", parts[0], parts[1]),
            repo: parts[3].to_string(),
        })
    } else {
        None
    }
}

fn try_parse_azure_ssh(url: &str) -> Option<RemoteUrlInfo> {
    let prefix = "git@ssh.dev.azure.com:v3/";
    let rest = url.strip_prefix(prefix)?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() >= 3 {
        Some(RemoteUrlInfo {
            provider: ProviderName::AzureDevOps,
            host: "dev.azure.com".to_string(),
            owner: format!("{}/{}", parts[0], parts[1]),
            repo: parts[2].to_string(),
        })
    } else {
        None
    }
}

fn try_parse_azure_legacy(url: &str) -> Option<RemoteUrlInfo> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host_part, path) = rest.split_once('/')?;
    let host = host_part.strip_suffix(".visualstudio.com")?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[1] == "_git" {
        Some(RemoteUrlInfo {
            provider: ProviderName::AzureDevOps,
            host: format!("{host}.visualstudio.com"),
            owner: format!("{host}/{}", parts[0]),
            repo: parts[2].to_string(),
        })
    } else {
        None
    }
}

fn try_parse_https(url: &str) -> Option<RemoteUrlInfo> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 2 {
        return None;
    }
    let repo = segments.pop()?.to_string();
    let owner = segments.join("/");
    Some(RemoteUrlInfo {
        provider: detect_provider(url),
        host: host.to_string(),
        owner,
        repo,
    })
}

fn try_parse_ssh_url(url: &str) -> Option<RemoteUrlInfo> {
    let rest = url.strip_prefix("ssh://git@")?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (host_port, path_part) = rest.split_once('/')?;
    let host = host_port.split(':').next().unwrap_or(host_port);
    let mut segments: Vec<&str> = path_part.split('/').collect();
    if segments.len() < 2 {
        return None;
    }
    let repo = segments.pop()?.to_string();
    let owner = segments.join("/");
    Some(RemoteUrlInfo {
        provider: detect_provider(url),
        host: host.to_string(),
        owner,
        repo,
    })
}

fn try_parse_ssh_scp(url: &str) -> Option<RemoteUrlInfo> {
    let rest = url.strip_prefix("git@")?;
    let (host, path) = rest.split_once(':')?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 2 {
        return None;
    }
    let repo = segments.pop()?.to_string();
    let owner = segments.join("/");
    Some(RemoteUrlInfo {
        provider: detect_provider(url),
        host: host.to_string(),
        owner,
        repo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_github() {
        assert_eq!(
            detect_provider("https://github.com/owner/repo"),
            ProviderName::GitHub
        );
    }

    #[test]
    fn detect_gitlab() {
        assert_eq!(
            detect_provider("https://gitlab.com/owner/repo"),
            ProviderName::GitLab
        );
    }

    #[test]
    fn detect_bitbucket() {
        assert_eq!(
            detect_provider("https://bitbucket.org/owner/repo"),
            ProviderName::Bitbucket
        );
    }

    #[test]
    fn detect_azure_devops() {
        assert_eq!(
            detect_provider("https://dev.azure.com/org/project/_git/repo"),
            ProviderName::AzureDevOps
        );
    }

    #[test]
    fn detect_gitea_selfhosted() {
        assert_eq!(
            detect_provider("https://gitea.example.com/owner/repo"),
            ProviderName::Gitea
        );
    }

    #[test]
    fn detect_forgejo_selfhosted() {
        assert_eq!(
            detect_provider("https://forgejo.example.com/owner/repo"),
            ProviderName::Forgejo
        );
    }

    #[test]
    fn parse_https_url() {
        let info = parse_remote_url("https://github.com/owner/repo.git").unwrap();
        assert_eq!(info.provider, ProviderName::GitHub);
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
    }

    #[test]
    fn parse_ssh_scp() {
        let info = parse_remote_url("git@github.com:owner/repo.git").unwrap();
        assert_eq!(info.provider, ProviderName::GitHub);
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
    }

    #[test]
    fn parse_azure_https() {
        let info = parse_remote_url("https://dev.azure.com/myorg/myproject/_git/myrepo").unwrap();
        assert_eq!(info.provider, ProviderName::AzureDevOps);
        assert_eq!(info.owner, "myorg/myproject");
        assert_eq!(info.repo, "myrepo");
    }

    #[test]
    fn parse_nested_groups() {
        let info = parse_remote_url("https://gitlab.com/group/subgroup/repo").unwrap();
        assert_eq!(info.owner, "group/subgroup");
        assert_eq!(info.repo, "repo");
    }
}
