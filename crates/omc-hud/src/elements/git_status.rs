use std::fs;
use std::path::{Path, PathBuf};

use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cwd = Path::new(ctx.input.cwd.as_deref().unwrap_or("."));
    let git_dir = find_git_dir(cwd)?;
    let branch = read_branch(&git_dir).unwrap_or_else(|| "?".to_string());
    let dirty = is_dirty(&git_dir);
    Some(format!("⎇ {branch}{}", if dirty { " *" } else { "" }))
}

fn find_git_dir(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            let raw = fs::read_to_string(&dot_git).ok()?;
            let path = raw.trim().strip_prefix("gitdir:")?.trim();
            let candidate = if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                dir.join(path)
            };
            return Some(candidate);
        }
    }
    None
}

fn read_branch(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(name) = head.strip_prefix("ref: refs/heads/") {
        Some(name.to_string())
    } else {
        Some(head.chars().take(7).collect())
    }
}

fn is_dirty(git_dir: &Path) -> bool {
    let index = git_dir.join("index");
    let head = git_dir.join("HEAD");
    let Ok(index_meta) = fs::metadata(index) else {
        return false;
    };
    let Ok(head_meta) = fs::metadata(head) else {
        return true;
    };
    match (index_meta.modified(), head_meta.modified()) {
        (Ok(index_time), Ok(head_time)) => index_time > head_time,
        _ => true,
    }
}
