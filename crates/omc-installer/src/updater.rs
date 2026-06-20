use thiserror::Error;

use crate::config::{InstallerPaths, VersionMetadata};

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Version parse error: {0}")]
    VersionParse(String),
    #[error("Update failed: {0}")]
    Other(String),
}

/// Semantic version parsed from a string like "1.2.3".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(input: &str) -> Result<Self, UpdateError> {
        let stripped = input.trim_start_matches('v');
        let parts: Vec<&str> = stripped.split('.').collect();
        if parts.len() < 3 {
            return Err(UpdateError::VersionParse(input.to_string()));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| UpdateError::VersionParse(input.to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| UpdateError::VersionParse(input.to_string()))?;
        let patch = parts[2]
            .split('-')
            .next()
            .unwrap_or("0")
            .parse()
            .map_err(|_| UpdateError::VersionParse(input.to_string()))?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Checks for OMC updates and manages the update lifecycle.
pub struct Updater {
    paths: InstallerPaths,
}

impl Updater {
    pub fn new() -> Option<Self> {
        let paths = InstallerPaths::default_config()?;
        Some(Self { paths })
    }

    pub fn with_paths(paths: InstallerPaths) -> Self {
        Self { paths }
    }

    /// Read the currently installed version from metadata.
    pub fn installed_version(&self) -> Option<String> {
        let content = std::fs::read_to_string(&self.paths.version_file).ok()?;
        let meta: VersionMetadata = serde_json::from_str(&content).ok()?;
        Some(meta.version)
    }

    /// Compare two semver version strings.
    /// Returns positive if a > b, negative if a < b, 0 if equal.
    pub fn compare_versions(a: &str, b: &str) -> Result<i32, UpdateError> {
        let va = SemVer::parse(a)?;
        let vb = SemVer::parse(b)?;
        if va > vb {
            Ok(1)
        } else if va < vb {
            Ok(-1)
        } else {
            Ok(0)
        }
    }

    /// Fetch the latest release version from GitHub.
    /// Uses the GitHub API: GET /repos/{owner}/{repo}/releases/latest
    pub async fn fetch_latest_version(&self) -> Result<String, UpdateError> {
        let url = "https://api.github.com/repos/2233admin/oh-my-claudecode-RS/releases/latest";
        let client = reqwest::Client::builder()
            .user_agent("omc-installer")
            .build()?;
        let resp = client.get(url).send().await?;
        let body: serde_json::Value = resp.json().await?;
        let tag = body["tag_name"]
            .as_str()
            .ok_or_else(|| UpdateError::Other("Missing tag_name in response".to_string()))?;
        Ok(tag.trim_start_matches('v').to_string())
    }

    /// Check if an update is available by comparing installed vs. latest.
    pub async fn check_update(&self) -> Result<Option<String>, UpdateError> {
        let installed = match self.installed_version() {
            Some(v) => v,
            None => return Ok(None),
        };
        let latest = self.fetch_latest_version().await?;
        if Self::compare_versions(&latest, &installed)? > 0 {
            Ok(Some(latest))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parse() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn semver_parse_with_prefix() {
        let v = SemVer::parse("v0.10.5").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 10);
        assert_eq!(v.patch, 5);
    }

    #[test]
    fn semver_parse_with_prerelease() {
        let v = SemVer::parse("1.0.0-beta").unwrap();
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn semver_ordering() {
        assert!(SemVer::parse("1.0.1").unwrap() > SemVer::parse("1.0.0").unwrap());
        assert!(SemVer::parse("2.0.0").unwrap() > SemVer::parse("1.9.9").unwrap());
        assert_eq!(
            SemVer::parse("1.2.3").unwrap(),
            SemVer::parse("1.2.3").unwrap()
        );
    }

    #[test]
    fn compare_versions_works() {
        assert_eq!(Updater::compare_versions("1.0.1", "1.0.0").unwrap(), 1);
        assert_eq!(Updater::compare_versions("1.0.0", "1.0.1").unwrap(), -1);
        assert_eq!(Updater::compare_versions("1.0.0", "1.0.0").unwrap(), 0);
    }
}
