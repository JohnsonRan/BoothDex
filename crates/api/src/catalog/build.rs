//! `Catalog` construction from `DocInput` rows and `TagWords`.

use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;

use super::{AVATAR_CATEGORY, Catalog, DocInput, Index};
use crate::expansion::Equivalents;
use crate::tag_words::TagWords;
use crate::text::{TextDoc, TextIndex};

impl Catalog {
    /// A catalogue without tag translations, for tests.
    #[cfg(test)]
    pub fn build(docs: Vec<DocInput>, now: i64, text_dir: Option<&std::path::Path>) -> Result<Self> {
        Self::build_with_tag_words(docs, &TagWords::default(), now, text_dir)
    }

    /// `tag_words`: the BOOTH tag dictionary, whose words of each item's tags its text search matches as words.
    pub fn build_with_tag_words(docs: Vec<DocInput>, tag_words: &TagWords, now: i64, text_dir: Option<&std::path::Path>) -> Result<Self> {
        let n = docs.len();
        let mut hasher = DefaultHasher::new();
        docs.len().hash(&mut hasher);
        for d in &docs {
            d.id.hash(&mut hasher);
            d.title.hash(&mut hasher);
            d.titles_translated.hash(&mut hasher);
            d.title_zh.hash(&mut hasher);
            d.title_is_zh.hash(&mut hasher);
            d.booth_tags.hash(&mut hasher);
            d.author.hash(&mut hasher);
            d.bio.hash(&mut hasher);
        }
        tag_words.fingerprint().hash(&mut hasher);
        let fingerprint = hasher.finish();

        let mut c = Catalog {
            ids: Vec::with_capacity(n),
            doc_of: FxHashMap::with_capacity_and_hasher(n, Default::default()),
            price: Vec::with_capacity(n),
            published: Vec::with_capacity(n),
            likes: Vec::with_capacity(n),
            trending: Vec::with_capacity(n),
            title_rank: vec![0; n],
            all: RoaringBitmap::new(),
            avatar_items: RoaringBitmap::new(),
            supported: RoaringBitmap::new(),
            adult_only: RoaringBitmap::new(),
            suggestive: RoaringBitmap::new(),
            relisted: RoaringBitmap::new(),
            avatars: Index::default(),
            authors: Index::default(),
            smart_tags: Index::default(),
            booth_tags: Index::default(),
            booth_cats: Index::default(),
            cats: Index::default(),
            cats_primary: Index::default(),
            features: Index::default(),
            gender: Index::default(),
            text: TextIndex::build(
                docs.iter().map(|d| TextDoc {
                    title: std::iter::once(&d.title).filter(|_| !d.title_is_zh).chain(&d.titles_translated).cloned().collect::<Vec<_>>().join("\n"),
                    title_zh: d.title_zh.clone().unwrap_or_default(),
                    tags: d.booth_tags.join("\n"),
                    tag_words: tag_words.text(&d.booth_tags),
                    author: d.author.clone(),
                    bio: d.bio.clone(),
                }),
                text_dir,
                fingerprint,
            )?,
            // built from the BOOTH tags below
            equivalents: Equivalents::build(&Index::default(), &RoaringBitmap::new(), &[], Default::default()),
        };

        let mut by_title: Vec<(String, u32)> = Vec::with_capacity(n);
        let mut titled: [RoaringBitmap; 4] = Default::default();
        for (i, d) in docs.into_iter().enumerate() {
            let i = i as u32;
            c.ids.push(d.id);
            c.doc_of.insert(d.id, i);
            c.price.push(d.price);
            let published = d.published.unwrap_or(0);
            c.published.push(published);
            c.likes.push(d.likes);
            // likes decayed by age (HN-style); VRCatalogue's own formula is unknown
            let age_days = ((now - published).max(0) as f32) / 86_400.0;
            c.trending.push(d.likes as f32 / (age_days + 2.0).powf(1.5));
            by_title.push((d.title.to_lowercase(), i));
            for (k, b) in titled.iter_mut().enumerate() {
                if d.title_languages & (1 << k) != 0 {
                    b.insert(i);
                }
            }

            c.all.insert(i);
            if d.booth_category.as_deref() == Some(AVATAR_CATEGORY) {
                c.avatar_items.insert(i);
            }
            if d.supported {
                c.supported.insert(i);
            }
            if d.relisted {
                c.relisted.insert(i);
            }
            let rating = d.rating.as_deref();
            if d.is_adult || rating == Some("NSFW") {
                c.adult_only.insert(i);
            } else if rating == Some("Suggestive") {
                c.suggestive.insert(i);
            }
            d.avatars.iter().for_each(|a| c.avatars.add(a, i));
            c.authors.add(&d.author, i);
            d.smart_tags.iter().for_each(|t| c.smart_tags.add(t, i));
            d.booth_tags.iter().for_each(|t| c.booth_tags.add(t, i));
            if let Some(bc) = &d.booth_category {
                c.booth_cats.add(bc, i);
            }
            for (slug, primary) in &d.categories {
                c.cats.add(slug, i);
                if *primary {
                    c.cats_primary.add(slug, i);
                }
            }
            d.features.iter().for_each(|f| c.features.add(f, i));
            if let Some(g) = &d.gender {
                c.gender.add(g, i);
            }
        }
        by_title.sort_unstable();
        for (rank, (_, i)) in by_title.into_iter().enumerate() {
            c.title_rank[i as usize] = rank as u32;
        }
        let mut shop_of = vec![0; n];
        for (shop, (_, docs)) in c.authors.entries().enumerate() {
            docs.iter().for_each(|d| shop_of[d as usize] = shop as u32);
        }
        c.equivalents = Equivalents::build(&c.booth_tags, &c.relisted, &shop_of, titled);
        Ok(c)
    }
}
