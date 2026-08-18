//! Shared dependency discovery for catalog and unified status surfaces.

use super::DependencyMetadata;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEvidence {
    pub id: String,
    pub available: bool,
    pub selected_command: Option<String>,
    pub source: &'static str,
}

pub fn dependency_evidence(metadata: &DependencyMetadata) -> DependencyEvidence {
    let selected_command = metadata
        .commands
        .iter()
        .find(|command| command_available(command))
        .cloned();
    DependencyEvidence {
        id: metadata.id.clone(),
        available: selected_command.is_some(),
        selected_command,
        source: "cataloged",
    }
}

fn command_available(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|path| {
            path.join(command).is_file()
                || (cfg!(windows) && path.join(format!("{command}.exe")).is_file())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_dependency_retains_cataloged_provenance() {
        let evidence = dependency_evidence(&DependencyMetadata {
            id: "definitely-absent-omc-test".into(),
            description: "test".into(),
            commands: vec!["definitely-absent-omc-test".into()],
            platforms: vec![],
        });
        assert_eq!(evidence.source, "cataloged");
        assert!(!evidence.available);
    }
}
