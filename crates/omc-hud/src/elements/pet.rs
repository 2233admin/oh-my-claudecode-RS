use crate::cache::now_ms;
use crate::terminal::ColorLevel;

#[derive(Clone, Copy)]
pub enum PetMood {
    Sleep,
    Idle,
    Busy,
    Danger,
    Panic,
}

pub struct PetFrame {
    pub lines: [String; 3],
    /// Visual width of the widest raw line (no ANSI), for layout padding.
    pub width: usize,
}

pub fn get_mood(ctx_pct: u8) -> PetMood {
    match ctx_pct {
        0..=14  => PetMood::Sleep,
        15..=49 => PetMood::Idle,
        50..=69 => PetMood::Busy,
        70..=84 => PetMood::Danger,
        _       => PetMood::Panic,
    }
}

fn anim_frame() -> usize {
    (now_ms() / 200) as usize % 4
}

/// Left/right eye chars, animated per mood.
fn eye_chars(mood: PetMood, frame: usize) -> (char, char) {
    match mood {
        PetMood::Sleep  => ('-', '-'),
        PetMood::Idle   => if frame == 1 { ('-', '-') } else { ('o', 'o') },
        PetMood::Busy   => ('^', '^'),
        PetMood::Danger => if frame % 2 == 0 { ('O', 'O') } else { ('-', 'O') },
        PetMood::Panic  => ('O', 'O'),
    }
}

/// Animated tail suffix appended to the paws line.
fn tail_str(mood: PetMood, frame: usize) -> &'static str {
    match mood {
        PetMood::Sleep  => ["z", "Z", "z", " "][frame],
        PetMood::Idle   => [" ", "~", " ", "~"][frame],
        PetMood::Busy   => ["~", "!", "~", "!"][frame],
        PetMood::Danger => ["!", " ", "!", " "][frame],
        PetMood::Panic  => ["!!", "!", "!!", "!"][frame],
    }
}

fn mood_color(mood: PetMood) -> &'static str {
    match mood {
        PetMood::Sleep                   => "\x1b[2m",
        PetMood::Idle                    => "\x1b[32m",
        PetMood::Busy                    => "\x1b[33m",
        PetMood::Danger | PetMood::Panic => "\x1b[31m",
    }
}

/// Build the 3 raw (no-ANSI) cat lines for a given size tier (0=tiny..4=thicc).
///
/// Size tier controls how many `=` pads the sides and how much inner spacing
/// the face has — matches codachi's cat scaling.
fn build_cat(size: usize, le: char, re: char, tail: &str) -> ([String; 3], usize) {
    let pad = "=".repeat(size);
    // inner spacing between eye and nose (1 space for tiny, 2 for small, ...)
    let sp = " ".repeat(size + 1);
    // paw inner = same visual width as `{le}{sp}w{sp}{re}` = 2*(size+1)+3
    let paw_sp = " ".repeat(2 * (size + 1) + 1);
    let underscores = "_".repeat(size * 2 + 1);

    let ears = format!("/\\{underscores}/\\");
    let face = format!("{pad}( {le}{sp}w{sp}{re} ){pad}");
    let paws = format!("{pad}( \"{paw_sp}\" ){pad}{tail}");

    let w = [ears.chars().count(), face.chars().count(), paws.chars().count()]
        .into_iter()
        .max()
        .unwrap_or(0);

    ([ears, face, paws], w)
}

pub fn render_pet(ctx_pct: Option<u8>, color_level: ColorLevel) -> PetFrame {
    let pct = ctx_pct.unwrap_or(0);
    let mood = get_mood(pct);
    let size = match pct {
        0..=19  => 0,
        20..=39 => 1,
        40..=59 => 2,
        60..=79 => 3,
        _       => 4,
    };
    let frame = anim_frame();
    let (le, re) = eye_chars(mood, frame);
    let tail = tail_str(mood, frame);

    let ([l0, l1, l2], width) = build_cat(size, le, re, tail);

    if matches!(color_level, ColorLevel::Mono) {
        PetFrame { lines: [l0, l1, l2], width }
    } else {
        let c = mood_color(mood);
        PetFrame {
            lines: [
                format!("{c}{l0}\x1b[0m"),
                format!("{c}{l1}\x1b[0m"),
                format!("{c}{l2}\x1b[0m"),
            ],
            width,
        }
    }
}
