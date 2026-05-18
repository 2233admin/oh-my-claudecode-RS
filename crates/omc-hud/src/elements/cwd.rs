use crate::elements::RenderContext;

const MAX_LEN: usize = 30;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cwd = ctx.input.cwd.as_deref()?;
    if cwd.is_empty() {
        return None;
    }

    // Replace home dir with ~
    let shortened = shorten_home(cwd);

    // Truncate to MAX_LEN with leading ellipsis
    let result = if shortened.len() > MAX_LEN {
        let truncated = &shortened[shortened.len() - MAX_LEN..];
        // Try to truncate at a path separator
        if let Some(pos) = truncated.find('/').or_else(|| truncated.find('\\')) {
            format!("...{}", &truncated[pos..])
        } else {
            format!("...{truncated}")
        }
    } else {
        shortened
    };

    Some(result)
}

fn shorten_home(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        // Normalize separators for comparison
        let path_norm = path.replace('\\', "/");
        let home_norm = home_str.replace('\\', "/");
        if path_norm.starts_with(home_norm.as_str()) {
            let rest = &path_norm[home_norm.len()..];
            return format!("~{rest}");
        }
    }
    path.replace('\\', "/")
}
