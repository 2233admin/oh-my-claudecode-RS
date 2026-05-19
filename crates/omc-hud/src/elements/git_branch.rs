use crate::elements::RenderContext;
use crate::git_util::git_output_timeout;
use crate::terminal::ColorLevel;
use std::time::Duration;

fn get_branch(cwd: &str) -> Option<String> {
    let output = git_output_timeout(
        &["-C", cwd, "branch", "--show-current"],
        Duration::from_millis(2000),
    )?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8(output.stdout).ok()?;
    let branch = branch.trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cwd = ctx.input.current_dir()?;
    let branch = get_branch(cwd)?;

    // Worktree name: prefer stdin.workspace.git_worktree, then stdin.worktree fields
    let wt_name = ctx
        .input
        .workspace
        .as_ref()
        .and_then(|w| w.git_worktree.as_deref())
        .or_else(|| ctx.input.worktree.as_ref().and_then(|w| w.name.as_deref()));

    if matches!(ctx.color_level, ColorLevel::Mono) {
        if let Some(wt) = wt_name {
            return Some(format!("branch:{branch} (wt:{wt})"));
        }
        return Some(format!("branch:{branch}"));
    }

    let mut s = format!("\x1b[2mbranch:\x1b[0m\x1b[36m{branch}\x1b[0m");
    if let Some(wt) = wt_name {
        s.push_str(&format!(
            " \x1b[2m(wt:\x1b[0m\x1b[36m{wt}\x1b[0m\x1b[2m)\x1b[0m"
        ));
    }
    Some(s)
}
