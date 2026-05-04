use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let max = ctx.input.context_window_max?;
    let current = ctx.input.context_window_tokens?;
    if current >= max {
        return Some("~0m".to_string());
    }

    let samples = &ctx.cache.context_samples;
    if samples.len() < 2 {
        return None;
    }

    let n = samples.len() as f64;
    let first_ts = samples.first()?.ts_ms as f64;
    let sum_x: f64 = samples.iter().map(|s| s.ts_ms as f64 - first_ts).sum();
    let sum_y: f64 = samples.iter().map(|s| s.tokens as f64).sum();
    let mean_x = sum_x / n;
    let mean_y = sum_y / n;

    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for sample in samples {
        let x = sample.ts_ms as f64 - first_ts;
        let y = sample.tokens as f64;
        numerator += (x - mean_x) * (y - mean_y);
        denominator += (x - mean_x).powi(2);
    }

    if denominator <= f64::EPSILON {
        return None;
    }
    let tokens_per_ms = numerator / denominator;
    if tokens_per_ms <= 0.0 || !tokens_per_ms.is_finite() {
        return None;
    }

    let remaining = (max - current) as f64;
    let eta_ms = remaining / tokens_per_ms;
    if !eta_ms.is_finite() {
        return None;
    }
    Some(format!("~{}", format_duration(eta_ms.max(0.0) as u64)))
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    if mins < 60 {
        format!("{mins}m")
    } else {
        format!("{}h", mins / 60)
    }
}
