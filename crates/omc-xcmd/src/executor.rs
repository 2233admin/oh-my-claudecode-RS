//! Execute x-cmd commands

use std::process::Command;

/// Execute x-cmd command and return output
pub fn run_xcmd(args: &[&str]) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;

    let x_script = home.join(".x-cmd.root/X");

    if !x_script.exists() {
        return Err("x-cmd not installed".to_string());
    }

    let x_args = if args.is_empty() {
        "x -h".to_string()
    } else {
        format!("x {}", args.join(" "))
    };

    let output = Command::new("bash")
        .args(["-c", &format!(". \"{}\" && {}", x_script.display(), x_args)])
        .output()
        .map_err(|e| format!("Failed to execute: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Count installed packages
pub fn count_packages() -> Option<usize> {
    let home = dirs::home_dir()?;
    let lock_dir = home.join(".x-cmd.root/env/lock");

    if !lock_dir.exists() {
        return Some(0);
    }

    std::fs::read_dir(&lock_dir).ok().map(|entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .count()
    })
}

/// Get package list
pub fn list_packages() -> Vec<String> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };

    let lock_dir = home.join(".x-cmd.root/env/lock");

    if !lock_dir.exists() {
        return vec![];
    }

    let mut packages = vec![];
    if let Ok(entries) = std::fs::read_dir(&lock_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry
                .path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
                && let Some(name) = entry.path().file_stem()
            {
                packages.push(name.to_string_lossy().to_string());
            }
        }
    }
    packages.sort();
    packages
}
