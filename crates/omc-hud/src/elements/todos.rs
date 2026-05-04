use std::fs;

use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    if let Some(count) = count_from_hooks(ctx) {
        return Some(format!("{count} {}", ctx.strings.todo));
    }
    if ctx.input.turns.is_some() && ctx.input.transcript_path.is_none() {
        return Some(format!("? {}", ctx.strings.todo));
    }

    let path = ctx.input.transcript_path.as_deref()?;
    let raw = fs::read_to_string(path).ok()?;
    let count = raw.matches("TODO").count() + raw.matches("FIXME").count();
    Some(format!("{count} {}", ctx.strings.todo))
}

fn count_from_hooks(ctx: &RenderContext<'_>) -> Option<usize> {
    let state = ctx.input.hooks_state.as_ref()?;
    for key in ["todos", "todo_count", "TODO"] {
        if let Some(value) = state.get(key).and_then(|v| v.as_u64()) {
            return Some(value as usize);
        }
        if let Some(array) = state.get(key).and_then(|v| v.as_array()) {
            return Some(array.len());
        }
    }
    None
}
