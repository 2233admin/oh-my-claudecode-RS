use std::process::Command;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn get_repo_name(cwd: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", cwd, "remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8(output.stdout).ok()?;
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    // Extract repo name from https://github.com/user/repo.git or git@github.com:user/repo.git
    let name = url.rsplit('/').next()
        .or_else(|| url.rsplit(':').next())?;
    let name = name.trim_end_matches(".git");
    if name.is_empty() { None } else { Some(name.to_string()) }
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cwd = ctx.input.current_dir()?;
    let name = get_repo_name(cwd)?;

    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(format!("repo:{name}"));
    }

    Some(format!("\x1b[2mrepo:\x1b[0m\x1b[36m{name}\x1b[0m"))
}
