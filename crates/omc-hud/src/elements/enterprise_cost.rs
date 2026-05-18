use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn severity_color(pct: u8) -> &'static str {
    if pct >= 90 { "\x1b[31m" } else if pct >= 70 { "\x1b[33m" } else { "\x1b[32m" }
}

fn format_money(amount: f64) -> String {
    let s = format!("{:.2}", amount);
    let (int_part, dec_part) = s.split_once('.').unwrap_or((&s, "00"));
    let with_commas: String = int_part
        .chars()
        .rev()
        .enumerate()
        .flat_map(|(i, c)| if i > 0 && i % 3 == 0 { vec![',', c] } else { vec![c] })
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{}.{}", with_commas, dec_part)
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let usage = ctx.usage?;
    let mono = matches!(ctx.color_level, ColorLevel::Mono);

    // Enterprise path: server-side spend tracking
    if let Some(spent) = usage.enterprise_used_usd {
        let spent_str = format_money(spent);
        return Some(match usage.enterprise_limit_usd.filter(|&l| l > 0.0) {
            None => {
                if mono { format!("spent:${}", spent_str) }
                else { format!("\x1b[2mspent:\x1b[0m${}", spent_str) }
            }
            Some(limit) => {
                let limit_str = format_money(limit);
                let pct = (spent / limit * 100.0).clamp(0.0, 100.0) as u8;
                if mono { format!("spent:${spent_str}/${limit_str} ({pct}%)") }
                else {
                    let c = severity_color(pct);
                    format!("\x1b[2mspent:\x1b[0m${spent_str}/${limit_str} {c}({pct}%)\x1b[0m")
                }
            }
        });
    }

    // Pro metered extra usage path
    let used = usage.extra_used_usd?;
    let limit = usage.extra_limit_usd?;
    if limit <= 0.0 { return None; }

    let pct = (used / limit * 100.0).clamp(0.0, 100.0) as u8;
    let used_str = format!("{:.2}", used);
    let limit_str = format!("{:.2}", limit);

    if mono {
        Some(format!("extra:{pct}%(${used_str}/${limit_str})"))
    } else {
        let c = severity_color(pct);
        Some(format!(
            "\x1b[2mextra:\x1b[0m{c}{pct}%\x1b[0m\x1b[2m(${used_str}/${limit_str})\x1b[0m"
        ))
    }
}
