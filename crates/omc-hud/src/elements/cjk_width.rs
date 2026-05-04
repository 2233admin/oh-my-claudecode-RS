//! `cjk_width` — placeholder element.
//!
//! East Asian wide / fullwidth / emoji characters occupy 2 terminal cells
//! while most other characters occupy 1. This element exists as a slot in
//! the [`super::Element`] enum so a future column-aware layout can name it
//! without special-casing, but it produces no visible output today.
//!
//! When a future element needs cell-width math, import the
//! [`unicode-width`](https://crates.io/crates/unicode-width) crate
//! directly (already a workspace dependency). Do not route through this
//! module — keep the indirection cost out of the hot path.

/// Placeholder element — never produces visible output.
pub fn render() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_always_returns_none() {
        assert_eq!(render(), None);
    }
}
