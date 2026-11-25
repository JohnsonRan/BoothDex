//! Reciprocal rank fusion combining lexical, semantic, tag-translation and category hits.

use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;

/// Reciprocal rank fusion: each list contributes 1 / (60 + rank), robust to the scores' different scales.
/// Meaning-only matches and matches of the query's translations (`translated`, which holds no text hit) count
/// half: they add recall, but a word match is the stronger signal, so such an item ranks with text hits around
/// 60th place unless two lists agree on it. Items in a category the query names get a little more than the best
/// meaning-only match: below the top text hits, above meaning-only ones.
pub(super) fn fuse(text: &FxHashMap<u32, f32>, semantic: &[u32], translated: &FxHashMap<u32, f32>, category: &RoaringBitmap) -> FxHashMap<u32, f32> {
    const K: f32 = 60.0;
    let mut out = FxHashMap::with_capacity_and_hasher(text.len() + semantic.len() + translated.len(), Default::default());
    for (d, rank) in ranks(text) {
        *out.entry(d).or_default() += 1.0 / (K + rank as f32 + 1.0);
    }
    for (d, rank) in semantic.iter().enumerate().map(|(rank, d)| (*d, rank)).chain(ranks(translated)) {
        *out.entry(d).or_default() += 0.5 / (K + rank as f32 + 1.0);
    }
    for d in category {
        *out.entry(d).or_default() += 0.5 / K;
    }
    out
}

/// Each doc's rank by score, best first. Equal scores share the rank of the first of them (0, 1, 1, 3), so docs
/// scored alike stay tied after fusion and the relevance order puts the trending one first.
fn ranks(scores: &FxHashMap<u32, f32>) -> Vec<(u32, usize)> {
    let mut sorted: Vec<(u32, f32)> = scores.iter().map(|(d, s)| (*d, *s)).collect();
    sorted.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    let mut rank = 0;
    sorted
        .iter()
        .enumerate()
        .map(|(i, &(d, s))| {
            if i > 0 && s < sorted[i - 1].1 {
                rank = i;
            }
            (d, rank)
        })
        .collect()
}
