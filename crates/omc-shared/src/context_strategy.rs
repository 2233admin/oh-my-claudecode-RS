use std::collections::HashMap;

/// Pluggable context compression strategies.
#[async_trait::async_trait]
pub trait ContextStrategy: Send + Sync {
    /// Returns true if the context should be compacted given the current state.
    fn should_compact(
        &self,
        message_count: usize,
        estimated_tokens: usize,
        max_tokens: usize,
    ) -> bool;

    /// Human-readable name of this strategy.
    fn name(&self) -> &str;
}

/// Novelty-aware strategy: uses bigram entropy to decide when to compress.
/// High entropy = novel content, don't compress. Low entropy = repetitive, compress.
#[derive(Debug, Clone)]
pub struct NoveltyAwareStrategy {
    pub entropy_threshold: f64,
    pub min_messages: usize,
}

impl NoveltyAwareStrategy {
    pub fn new(entropy_threshold: f64, min_messages: usize) -> Self {
        Self {
            entropy_threshold,
            min_messages,
        }
    }

    /// Compute the Shannon entropy of bigrams in the given text.
    pub fn bigram_entropy(&self, text: &str) -> f64 {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() < 2 {
            return 0.0;
        }

        let mut freq: HashMap<(char, char), u64> = HashMap::new();
        let mut total = 0u64;
        for window in chars.windows(2) {
            let bigram = (window[0], window[1]);
            *freq.entry(bigram).or_insert(0) += 1;
            total += 1;
        }

        if total == 0 {
            return 0.0;
        }

        let total_f = total as f64;
        let mut entropy = 0.0_f64;
        for count in freq.values() {
            let p = *count as f64 / total_f;
            if p > 0.0 {
                entropy -= p * p.log2();
            }
        }

        entropy
    }
}

impl Default for NoveltyAwareStrategy {
    fn default() -> Self {
        Self(2.5, 10)
    }
}

#[async_trait::async_trait]
impl ContextStrategy for NoveltyAwareStrategy {
    fn should_compact(
        &self,
        message_count: usize,
        estimated_tokens: usize,
        max_tokens: usize,
    ) -> bool {
        if message_count < self.min_messages {
            return false;
        }
        if estimated_tokens < max_tokens {
            return false;
        }
        // At this point we're over capacity. Check if recent content is novel.
        // In a real implementation, this would receive recent message text.
        // For the trait interface, we fall back to token-based threshold.
        true
    }

    fn name(&self) -> &str {
        "novelty-aware"
    }
}

/// Simple threshold strategy: compact when tokens exceed `max_tokens * ratio`.
#[derive(Debug, Clone)]
pub struct ThresholdStrategy {
    pub ratio: f64,
}

impl ThresholdStrategy {
    pub fn new(ratio: f64) -> Self {
        Self { ratio }
    }
}

impl Default for ThresholdStrategy {
    fn default() -> Self {
        Self(0.8)
    }
}

#[async_trait::async_trait]
impl ContextStrategy for ThresholdStrategy {
    fn should_compact(
        &self,
        _message_count: usize,
        estimated_tokens: usize,
        max_tokens: usize,
    ) -> bool {
        let threshold = max_tokens as f64 * self.ratio;
        estimated_tokens as f64 > threshold
    }

    fn name(&self) -> &str {
        "threshold"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigram_entropy_repetitive_is_low() {
        let strategy = NoveltyAwareStrategy::default();
        let low = strategy.bigram_entropy("aaaa");
        let high = strategy.bigram_entropy("abcdefgh");
        assert!(
            low < high,
            "repetitive text entropy ({low}) should be less than varied text ({high})"
        );
    }

    #[test]
    fn bigram_entropy_empty_string() {
        let strategy = NoveltyAwareStrategy::default();
        assert_eq!(strategy.bigram_entropy(""), 0.0);
    }

    #[test]
    fn bigram_entropy_single_char() {
        let strategy = NoveltyAwareStrategy::default();
        assert_eq!(strategy.bigram_entropy("a"), 0.0);
    }

    #[test]
    fn bigram_entropy_all_same_bigrams() {
        let strategy = NoveltyAwareStrategy::default();
        // "aa" has exactly one bigram (a,a) -> entropy = 0
        assert_eq!(strategy.bigram_entropy("aa"), 0.0);
        // "aaaaa" also only has one unique bigram
        assert_eq!(strategy.bigram_entropy("aaaaa"), 0.0);
    }

    #[test]
    fn novelty_should_compact_low_messages_false() {
        let strategy = NoveltyAwareStrategy::new(2.5, 10);
        // Below min_messages threshold, never compact.
        assert!(!strategy.should_compact(5, 10000, 1000));
    }

    #[test]
    fn novelty_should_compact_under_token_limit_false() {
        let strategy = NoveltyAwareStrategy::new(2.5, 10);
        // Enough messages but under token limit.
        assert!(!strategy.should_compact(15, 500, 1000));
    }

    #[test]
    fn novelty_should_compact_over_token_limit_true() {
        let strategy = NoveltyAwareStrategy::new(2.5, 10);
        // Enough messages and over token limit.
        assert!(strategy.should_compact(15, 1200, 1000));
    }

    #[test]
    fn novelty_name() {
        let strategy = NoveltyAwareStrategy::default();
        assert_eq!(strategy.name(), "novelty-aware");
    }

    #[test]
    fn threshold_below_ratio_false() {
        let strategy = ThresholdStrategy::new(0.8);
        // 500 tokens < 800 (0.8 * 1000)
        assert!(!strategy.should_compact(0, 500, 1000));
    }

    #[test]
    fn threshold_above_ratio_true() {
        let strategy = ThresholdStrategy::new(0.8);
        // 900 tokens > 800 (0.8 * 1000)
        assert!(strategy.should_compact(0, 900, 1000));
    }

    #[test]
    fn threshold_exact_ratio_false() {
        let strategy = ThresholdStrategy::new(0.8);
        // Exactly at threshold: 800 is not > 800.
        assert!(!strategy.should_compact(0, 800, 1000));
    }

    #[test]
    fn threshold_name() {
        let strategy = ThresholdStrategy::default();
        assert_eq!(strategy.name(), "threshold");
    }

    #[test]
    fn threshold_default_ratio() {
        let strategy = ThresholdStrategy::default();
        assert!((strategy.ratio - 0.8).abs() < 1e-6);
    }

    #[test]
    fn novelty_default_values() {
        let strategy = NoveltyAwareStrategy::default();
        assert!((strategy.entropy_threshold - 2.5).abs() < 1e-6);
        assert_eq!(strategy.min_messages, 10);
    }
}
