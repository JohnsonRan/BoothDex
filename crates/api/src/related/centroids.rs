//! Per-BOOTH-tag mean embedding centroids and cosine ranking for related searches.

use std::sync::Arc;

use anyhow::Result;
use boothdex_core::embed::{DIMS, HalfVector};
use futures_util::TryStreamExt;
use sqlx::PgPool;

use crate::catalog::Catalog;

/// A tag needs this many embedded items for a centroid; it is on at least 20 listed items (expansion.rs).
const MIN_EMBEDDED: u32 = 5;
/// Centroids this old are built again at the next reload; younger ones carry over to the new snapshot.
pub(super) const REBUILD_AFTER: chrono::TimeDelta = chrono::TimeDelta::hours(6);

/// Per BOOTH tag, the mean of its items' embeddings, from one embedding model.
pub(crate) struct TagCentroids {
    /// the model of the item embeddings, as the worker records it (`app_settings` key `embed_state`)
    pub(super) model: String,
    pub(super) built: chrono::DateTime<chrono::Utc>,
    /// folded tags (`expansion::Equivalents` keys), and their centroids in the same order, `DIMS` floats each, unit
    /// length
    pub(super) keys: Vec<String>,
    pub(super) vectors: Vec<f32>,
}

impl TagCentroids {
    /// The centroids a new snapshot of `catalog` uses: `previous` while it is non-empty, from the current embedding
    /// model and younger than `REBUILD_AFTER`, else built again from every item embedding (in production, 2026-11-20,
    /// about 212k of them, 435 MB as halfvec). `None` when no item has been embedded.
    pub(crate) async fn current(pool: &PgPool, catalog: &Catalog, previous: Option<Arc<Self>>) -> Result<Option<Arc<Self>>> {
        // one view of the database: the worker clears the embeddings when the model changes, then records the model
        let mut tx = pool.begin().await?;
        sqlx::query("set transaction isolation level repeatable read, read only").execute(&mut *tx).await?;
        let model: Option<String> =
            sqlx::query_scalar("select value->>'model' from app_settings where key = 'embed_state'").fetch_optional(&mut *tx).await?.flatten();
        let Some(model) = model else { return Ok(None) };
        // never cache an empty build (e.g. a snapshot loaded right after a model switch while items are still being embedded)
        if let Some(p) = previous.filter(|p| !p.keys.is_empty() && p.model == model && chrono::Utc::now() - p.built < REBUILD_AFTER) {
            return Ok(Some(p));
        }
        // which tags get a centroid first, from the ids alone, so the sums take no memory for the others
        let embedded: Vec<i64> = sqlx::query_scalar("select id from items where embedding is not null").fetch_all(&mut *tx).await?;
        let mut sums = Sums::new(catalog, embedded);
        let mut rows = sqlx::query_as::<_, (i64, HalfVector)>("select id, embedding from items where embedding is not null").fetch(&mut *tx);
        while let Some((id, v)) = rows.try_next().await? {
            sums.add(id, v.as_slice().iter().map(|x| x.to_f32()));
        }
        Ok(Some(Arc::new(sums.finish(model, chrono::Utc::now()))))
    }

    pub(super) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    /// Heap memory, for the startup log.
    pub(crate) fn bytes(&self) -> usize {
        self.vectors.len() * size_of::<f32>() + self.keys.iter().map(|k| k.len() + size_of::<String>()).sum::<usize>()
    }

    /// The tags nearest to `v` by cosine similarity first (the centroids are unit length, so the dot product orders
    /// them); ties by the tag.
    pub(super) fn ranked(&self, v: &[f32]) -> impl Iterator<Item = &str> {
        let mut order: Vec<(f32, usize)> = self.vectors.as_chunks::<DIMS>().0.iter().map(|c| dot(c, v)).zip(0..).collect();
        order.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then_with(|| self.keys[a.1].cmp(&self.keys[b.1])));
        order.into_iter().map(|(_, i)| self.keys[i].as_str())
    }
}

/// Centroids being summed: each item's embedding, at unit length, added to each of its tags that gets a centroid.
pub(super) struct Sums<'a> {
    catalog: &'a Catalog,
    /// per tag, its row in `sums`: the tags on at least `MIN_EMBEDDED` embedded items
    rows: Vec<Option<u32>>,
    sums: Vec<f32>,
    v: Vec<f32>,
}

impl<'a> Sums<'a> {
    /// Rows for the tags that `embedded` (the ids of the embedded items) gives a centroid, so memory holds only those.
    pub(super) fn new(catalog: &'a Catalog, embedded: impl IntoIterator<Item = i64>) -> Self {
        let tags = catalog.tags();
        let mut counts = vec![0u32; tags.len()];
        for d in embedded.into_iter().filter_map(|id| catalog.doc(id)) {
            tags.tags_of(d).iter().for_each(|&t| counts[t as usize] += 1);
        }
        let mut n = 0;
        let rows = counts
            .into_iter()
            .map(|c| {
                (c >= MIN_EMBEDDED).then(|| {
                    n += 1;
                    n - 1
                })
            })
            .collect();
        Self { catalog, rows, sums: vec![0.0; n as usize * DIMS], v: vec![0.0; DIMS] }
    }

    pub(super) fn add(&mut self, id: i64, embedding: impl Iterator<Item = f32>) {
        let Some(tags) = self.catalog.doc(id).map(|d| self.catalog.tags().tags_of(d)).filter(|t| !t.is_empty()) else { return };
        self.v.iter_mut().zip(embedding).for_each(|(x, e)| *x = e);
        normalize(&mut self.v);
        for row in tags.iter().filter_map(|&t| self.rows[t as usize]) {
            self.sums[row as usize * DIMS..][..DIMS].iter_mut().zip(&self.v).for_each(|(s, x)| *s += x);
        }
    }

    /// The sums made unit length.
    pub(super) fn finish(self, model: String, built: chrono::DateTime<chrono::Utc>) -> TagCentroids {
        let Self { catalog, rows, mut sums, .. } = self;
        let mut keys = vec![String::new(); sums.len() / DIMS];
        for (key, t) in catalog.tags().keys() {
            if let Some(row) = rows[t as usize] {
                keys[row as usize] = key.to_owned();
            }
        }
        sums.chunks_mut(DIMS).for_each(normalize);
        TagCentroids { model, built, keys, vectors: sums }
    }
}

fn normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

/// In eight lanes, which the compiler vectorizes; `DIMS` is a multiple of 8.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut lanes = [0.0f32; 8];
    for (x, y) in a.as_chunks::<8>().0.iter().zip(b.as_chunks::<8>().0) {
        for i in 0..8 {
            lanes[i] += x[i] * y[i];
        }
    }
    lanes.iter().sum()
}
