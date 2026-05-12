use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single memory entry in the reasoning bank.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// The stored content (e.g. a strategy description, lesson learned).
    pub content: String,
    /// Embedding vector for similarity search.
    pub embedding: Vec<f32>,
    /// When this entry was created.
    pub created_at: DateTime<Utc>,
    /// Reliability score in [0.0, 1.0], updated via `judge`.
    pub reliability: f64,
    /// How many times this entry has been retrieved.
    pub access_count: u64,
    /// Outcome label (e.g. "success", "failure", "neutral").
    pub outcome: String,
}

/// 4-factor retrieval system with MMR diversity, based on agentic-flow's ReasoningBank.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningBank {
    entries: Vec<MemoryEntry>,
    /// Weight for similarity factor.
    alpha: f64,
    /// Weight for recency factor.
    beta: f64,
    /// Weight for reliability factor.
    gamma: f64,
}

impl ReasoningBank {
    pub fn new(alpha: f64, beta: f64, gamma: f64) -> Self {
        Self {
            entries: Vec::new(),
            alpha,
            beta,
            gamma,
        }
    }

    /// Store a new memory entry. Returns the index of the stored entry.
    pub fn store(&mut self, content: String, embedding: Vec<f32>, outcome: String) -> usize {
        let idx = self.entries.len();
        self.entries.push(MemoryEntry {
            content,
            embedding,
            created_at: Utc::now(),
            reliability: 0.5,
            access_count: 0,
            outcome,
        });
        idx
    }

    /// Retrieve the top-k entries by composite score (similarity + recency + reliability),
    /// filtered through MMR for diversity. Returns `(index, score)` pairs.
    pub fn retrieve(&mut self, query_embedding: &[f32], top_k: usize) -> Vec<(usize, f64)> {
        if self.entries.is_empty() || top_k == 0 {
            return Vec::new();
        }

        let now = Utc::now();
        let scored: Vec<(usize, f64)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let similarity = cosine_similarity(&entry.embedding, query_embedding);
                let age_hours = (now - entry.created_at).num_hours().max(0) as f64;
                let recency = 1.0 / (1.0 + age_hours);
                let score =
                    self.alpha * similarity + self.beta * recency + self.gamma * entry.reliability;
                (i, score)
            })
            .collect();

        let selected = mmr_select(&scored, top_k, 0.5);

        // Increment access counts for selected entries.
        for &idx in &selected {
            self.entries[idx].access_count += 1;
        }

        selected
            .into_iter()
            .filter_map(|idx| {
                scored
                    .iter()
                    .find(|(i, _)| *i == idx)
                    .map(|(_, s)| (idx, *s))
            })
            .collect()
    }

    /// Update the reliability of an entry based on an outcome.
    pub fn judge(&mut self, idx: usize, success: bool, confidence: f64) {
        if let Some(entry) = self.entries.get_mut(idx) {
            let delta = if success { confidence } else { -confidence };
            entry.reliability = (entry.reliability + delta).clamp(0.0, 1.0);
        }
    }

    /// Remove the lowest-scored entries when the bank exceeds `max_entries`.
    pub fn consolidate(&mut self, max_entries: usize) {
        if self.entries.len() <= max_entries {
            return;
        }

        let now = Utc::now();
        let mut scored: Vec<(usize, f64)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let age_hours = (now - entry.created_at).num_hours().max(0) as f64;
                let recency = 1.0 / (1.0 + age_hours);
                let score = self.beta * recency + self.gamma * entry.reliability;
                (i, score)
            })
            .collect();

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let remove_count = self.entries.len() - max_entries;
        let remove_indices: Vec<usize> =
            scored.iter().take(remove_count).map(|(i, _)| *i).collect();

        // Remove in reverse order to preserve indices.
        let mut sorted_remove = remove_indices;
        sorted_remove.sort_unstable();
        for idx in sorted_remove.into_iter().rev() {
            self.entries.swap_remove(idx);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b
        .iter()
        .map(|x| (*x as f64) * (*x as f64))
        .sum::<f64>()
        .sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// Maximal Marginal Relevance selection. Picks `k` entries from `scored` that maximize
/// a balance of relevance and diversity. `lambda` controls the tradeoff (1.0 = pure relevance).
pub fn mmr_select(scored: &[(usize, f64)], k: usize, lambda: f64) -> Vec<usize> {
    if scored.is_empty() || k == 0 {
        return Vec::new();
    }

    let k = k.min(scored.len());
    let mut selected: Vec<usize> = Vec::with_capacity(k);
    let mut remaining: Vec<(usize, f64)> = scored.to_vec();

    // Select the highest-scoring entry first.
    remaining.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let first = remaining.remove(0);
    selected.push(first.0);

    while selected.len() < k && !remaining.is_empty() {
        let mut best_idx = 0;
        let mut best_mmr = f64::NEG_INFINITY;

        for (ri, &(cand_idx, cand_score)) in remaining.iter().enumerate() {
            // Max similarity to any already-selected entry (approximate with score difference).
            let max_sim_to_selected = selected
                .iter()
                .map(|&sel_idx| {
                    // Use absolute index difference as a proxy for diversity.
                    // In a full implementation, this would use actual embeddings.
                    let diff = (sel_idx as f64 - cand_idx as f64).abs();
                    1.0 / (1.0 + diff)
                })
                .fold(0.0_f64, f64::max);

            let mmr = lambda * cand_score - (1.0 - lambda) * max_sim_to_selected;
            if mmr > best_mmr {
                best_mmr = mmr;
                best_idx = ri;
            }
        }

        let chosen = remaining.remove(best_idx);
        selected.push(chosen.0);
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = [1.0, 2.0, 3.0];
        let b = [1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "identical vectors should have similarity 1.0, got {sim}"
        );
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = [1.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            sim.abs() < 1e-6,
            "orthogonal vectors should have similarity 0.0, got {sim}"
        );
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = [0.0, 0.0];
        let b = [1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_mismatched_lengths() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }

    #[test]
    fn store_and_retrieve_roundtrip() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.0);
        let idx = bank.store("test content".into(), vec![1.0, 0.0, 0.0], "success".into());
        assert_eq!(idx, 0);

        let results = bank.retrieve(&[1.0, 0.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
        assert!(
            results[0].1 > 0.99,
            "exact match score should be ~1.0, got {}",
            results[0].1
        );
    }

    #[test]
    fn judge_updates_reliability() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.5);
        let idx = bank.store("entry".into(), vec![1.0], "neutral".into());
        let initial = bank.entries[idx].reliability;
        assert!((initial - 0.5).abs() < 1e-6);

        bank.judge(idx, true, 0.3);
        assert!((bank.entries[idx].reliability - 0.8).abs() < 1e-6);

        bank.judge(idx, false, 0.9);
        // 0.8 - 0.9 = -0.1, clamped to 0.0.
        assert!(bank.entries[idx].reliability.abs() < 1e-6);
    }

    #[test]
    fn retrieve_empty_bank() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.0);
        let results = bank.retrieve(&[1.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn retrieve_respects_top_k() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.0);
        for i in 0..10 {
            bank.store(format!("entry {i}"), vec![i as f32], "neutral".into());
        }
        let results = bank.retrieve(&[0.0], 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn consolidate_removes_low_scorers() {
        let mut bank = ReasoningBank::new(0.0, 0.5, 0.5);
        for i in 0..10 {
            let idx = bank.store(format!("entry {i}"), vec![i as f32], "neutral".into());
            // Make early entries more reliable.
            if i < 5 {
                bank.judge(idx, true, 0.5);
            }
        }
        assert_eq!(bank.len(), 10);
        bank.consolidate(5);
        assert_eq!(bank.len(), 5);
    }

    #[test]
    fn consolidate_noop_when_under_limit() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.0);
        bank.store("a".into(), vec![1.0], "neutral".into());
        bank.consolidate(10);
        assert_eq!(bank.len(), 1);
    }

    #[test]
    fn mmr_select_diversity() {
        // All entries have equal score. MMR should still select k entries.
        let scored: Vec<(usize, f64)> = (0..10).map(|i| (i, 1.0)).collect();
        let selected = mmr_select(&scored, 5, 0.5);
        assert_eq!(selected.len(), 5);
        // Verify all selected indices are unique.
        let mut sorted = selected.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 5);
    }

    #[test]
    fn mmr_select_empty_input() {
        let selected = mmr_select(&[], 5, 0.5);
        assert!(selected.is_empty());
    }

    #[test]
    fn mmr_select_k_larger_than_input() {
        let scored = vec![(0, 1.0), (1, 0.8)];
        let selected = mmr_select(&scored, 10, 0.5);
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn access_count_increments_on_retrieve() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.0);
        bank.store("entry".into(), vec![1.0], "neutral".into());
        assert_eq!(bank.entries[0].access_count, 0);

        bank.retrieve(&[1.0], 1);
        assert_eq!(bank.entries[0].access_count, 1);

        bank.retrieve(&[1.0], 1);
        assert_eq!(bank.entries[0].access_count, 2);
    }

    #[test]
    fn len_and_is_empty() {
        let mut bank = ReasoningBank::new(1.0, 0.0, 0.0);
        assert!(bank.is_empty());
        assert_eq!(bank.len(), 0);

        bank.store("a".into(), vec![1.0], "neutral".into());
        assert!(!bank.is_empty());
        assert_eq!(bank.len(), 1);
    }
}
