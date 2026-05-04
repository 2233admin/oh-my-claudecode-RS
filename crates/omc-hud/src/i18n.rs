use std::env;

#[derive(Debug, Clone, Copy)]
pub enum Locale {
    En,
    ZhCn,
}

#[derive(Debug, Clone, Copy)]
pub struct Strings {
    pub ctx: &'static str,
    pub tok: &'static str,
    pub todo: &'static str,
    pub autopilot: &'static str,
    pub rl: &'static str,
}

const EN: Strings = Strings {
    ctx: "CTX",
    tok: "tok",
    todo: "TODO",
    autopilot: "autopilot",
    rl: "RL",
};

const ZH_CN: Strings = Strings {
    ctx: "上下文",
    tok: "词元",
    todo: "待办",
    autopilot: "自动",
    rl: "限额",
};

pub fn detect_locale() -> Locale {
    let locale = env::var("LC_ALL").or_else(|_| env::var("LANG")).unwrap_or_default();
    if locale.to_ascii_lowercase().contains("zh") {
        Locale::ZhCn
    } else {
        Locale::En
    }
}

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::ZhCn => &ZH_CN,
    }
}
