//! Filtering, text/semantic/expansion fusion, disjunctive faceting, sorting and paging on `Catalog`.

use std::sync::Arc;

use anyhow::Result;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;

use super::fusion::fuse;
use super::{Catalog, FEATURES, Facets, Group, Index, PRICE_CAP, SearchResult, Semantic, TextHits};
use crate::search_request::{Adult, Range, SearchRequest, Sort, Support, View};
use crate::text::TextMatches;

impl Catalog {
    pub(super) fn constraints(&self, r: &SearchRequest, now: i64, text: Option<&TextHits>, favourites: Option<&RoaringBitmap>) -> Vec<(Group, RoaringBitmap)> {
        let mut out = Vec::new();
        let mut view = match (r.view, favourites) {
            (View::Favourites, Some(f)) => f.clone(), // favourites span both items and avatar bodies
            (View::Avatars(support), _) => &match support {
                Support::All => self.avatar_items.clone(),
                Support::Unsupported => &self.avatar_items - &self.supported,
                Support::Supported => &self.avatar_items & &self.supported,
            } - &self.relisted,
            _ => &(&self.all - &self.avatar_items) - &self.relisted,
        };
        // the item the query names shows in every view, an avatar or a re-uploaded item too: the person asked for that
        // id, and a re-uploaded item's dialog links the new listing
        if let Some(d) = text.and_then(|t| t.pinned) {
            view.insert(d);
        }
        out.push((Group::View, view));
        if let (View::Avatars(_), Some(g)) = (r.view, &r.gender) {
            out.push((Group::Gender, self.gender.any(std::slice::from_ref(g))));
        }
        match r.adult {
            Adult::Include => {}
            Adult::Only => out.push((Group::Adult, self.adult_only.clone())),
            Adult::Suggestive => out.push((Group::Adult, self.suggestive.clone())),
            Adult::Safe => out.push((Group::Adult, &self.all - &self.adult_only)), // "safe" keeps Suggestive, as VRCatalogue did
        }
        if let Some(t) = text {
            out.push((Group::Text, t.matches.hits.clone()));
        }
        let mut list = |g, ix: &Index, keys: &[String]| {
            if !keys.is_empty() {
                out.push((g, ix.any(keys)));
            }
        };
        list(Group::Avatars, &self.avatars, &r.avatars);
        list(Group::Authors, &self.authors, &r.authors);
        list(Group::Tags, &self.smart_tags, &r.tags);
        list(Group::BoothTags, &self.booth_tags, &r.btags);
        list(Group::BoothCat, &self.booth_cats, &r.bc);
        let cats = if r.scp { &self.cats_primary } else { &self.cats };
        list(Group::SmartCat, cats, &r.sc);
        // exclusions narrow the same group as their include list
        let exclude = |ix: &Index, keys: &[String]| (!keys.is_empty()).then(|| &self.all - &ix.any(keys));
        if let Some(b) = exclude(&self.smart_tags, &r.extags) {
            out.push((Group::Tags, b));
        }
        if let Some(b) = exclude(&self.booth_tags, &r.exbtags) {
            out.push((Group::BoothTags, b));
        }
        if !r.features.is_empty() {
            // features AND together
            out.push((Group::Features, r.features.iter().fold(self.all.clone(), |acc, f| acc & self.features.any(std::slice::from_ref(f)))));
        }

        if r.free {
            out.push((Group::Price, self.scan(|i| self.price[i] == Some(0))));
        } else if r.pmin > 0 || (r.pmax > 0 && r.pmax < PRICE_CAP) {
            let hi = if r.pmax > 0 && r.pmax < PRICE_CAP { r.pmax } else { i32::MAX };
            out.push((Group::Price, self.scan(|i| self.price[i].is_some_and(|v| v >= r.pmin && v <= hi))));
        }

        let day = 86_400;
        let range = r.range.map(|range| match range {
            Range::Days(n) => (now - n * day, i64::MAX),
            Range::Custom { from, to } => {
                let ts = |d: Option<chrono::NaiveDate>| d.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
                // `to` is inclusive of the whole day
                (ts(from).unwrap_or(i64::MIN), ts(to).map_or(i64::MAX, |t| t + day - 1))
            }
        });
        if let Some((lo, hi)) = range {
            out.push((Group::Range, self.scan(|i| self.published[i] >= lo && self.published[i] <= hi)));
        }
        out
    }

    pub(super) fn intersect(&self, cs: &[(Group, RoaringBitmap)], skip: Option<Group>) -> RoaringBitmap {
        cs.iter().filter(|(g, _)| Some(*g) != skip).fold(self.all.clone(), |acc, (_, b)| acc & b)
    }

    /// The text query's hits and scores, joined by the meaning-based matches when there are any, by the matches of
    /// the BOOTH tags that translate `expand` (expansion.rs) when it is given, and by the item the query names by its
    /// link or number, scored above them all.
    pub(super) fn text_hits(&self, r: &SearchRequest, semantic: Option<Semantic>, expand: Option<&str>) -> Result<Option<TextHits>> {
        let Some(q) = r.q.as_deref() else { return Ok(None) };
        let TextMatches { mut hits, mut scores, typo } = self.text.search(q)?;
        // models named in the query: a meaning-based or translated match must fit one of them
        let fits = semantic.filter(|sem| !sem.models.is_empty()).map(|sem| self.avatars.any(sem.models));
        if let (Some(sem), Some(f)) = (semantic, &fits) {
            // a named model also works like the avatar filter: its compatible items match even when they don't
            // spell the name (most write it in Japanese), the other words searched among them. Items matching the
            // whole query stay and score both, so they rank first.
            if sem.rest.trim().is_empty() {
                hits |= f;
            } else {
                let TextMatches { hits: rest_hits, scores: rest_scores, .. } = self.text.search(sem.rest)?;
                let rest_hits = rest_hits & f;
                for d in &rest_hits {
                    *scores.entry(d).or_default() += rest_scores.get(&d).copied().unwrap_or(0.0);
                }
                hits |= rest_hits;
            }
        }
        let expanded = match expand {
            Some(meaning) => self.equivalents(meaning)?,
            None => Arc::from([]),
        };
        // one vote per item, its best score over the equivalents; the query's own matches take none, so their
        // order among themselves stays
        let mut translated: FxHashMap<u32, f32> = FxHashMap::default();
        for e in expanded.iter() {
            let m = self.equivalents.matches(e, &self.text)?;
            for d in m.hits.iter().filter(|d| !hits.contains(*d) && fits.as_ref().is_none_or(|f| f.contains(*d))) {
                let s = m.scores.get(&d).copied().unwrap_or(0.0);
                let best = translated.entry(d).or_insert(s);
                *best = best.max(s);
            }
        }
        let expansion: RoaringBitmap = translated.keys().copied().collect();
        if semantic.is_some() || !translated.is_empty() {
            let (ids, categories) = semantic.map_or((&[][..], &[][..]), |sem| (sem.ids, sem.categories));
            let docs: Vec<u32> = ids.iter().filter_map(|id| self.doc_of.get(id)).copied().filter(|d| fits.as_ref().is_none_or(|f| f.contains(*d))).collect();
            let mut cat = self.cats.any(categories);
            if let Some(f) = &fits {
                cat &= f;
            }
            scores = fuse(&scores, &docs, &translated, &cat);
            hits.extend(docs);
            hits |= &expansion;
            hits |= cat;
        }
        let pinned = self.referenced(q).flatten();
        if let Some(d) = pinned {
            hits.insert(d);
            scores.insert(d, f32::INFINITY);
        }
        Ok(Some(TextHits { matches: TextMatches { hits, scores, typo }, expanded, expansion, pinned }))
    }

    /// How many items each of `wider` finds: requests that differ from `r` only in their filters (never in the
    /// text query), so `r`'s text, meaning-based and translated matches hold for all of them. Counts only: no
    /// facets, no order.
    pub fn counts(
        &self,
        r: &SearchRequest,
        wider: &[SearchRequest],
        now: i64,
        favourites: Option<&RoaringBitmap>,
        semantic: Option<Semantic>,
        expand: Option<&str>,
    ) -> Result<Vec<u64>> {
        let text = self.text_hits(r, semantic, expand)?;
        Ok(wider.iter().map(|w| self.intersect(&self.constraints(w, now, text.as_ref(), favourites), None).len()).collect())
    }

    /// `semantic`: item ids nearest to the query by meaning (see search.rs); they join the text hits and, when
    /// sorting by relevance, the two rankings are merged. `expand`: the words whose translations are searched too
    /// (expansion.rs), for `/api/search` only. Autocomplete and RSS skip facet counts.
    pub fn search(
        &self,
        r: &SearchRequest,
        now: i64,
        favourites: Option<&RoaringBitmap>,
        semantic: Option<Semantic>,
        expand: Option<&str>,
        count_facets: bool,
    ) -> Result<SearchResult> {
        let text = self.text_hits(r, semantic, expand)?;
        let cs = self.constraints(r, now, text.as_ref(), favourites);
        let hits = self.intersect(&cs, None);

        // disjunctive facets: each group's counts ignore that group's own selection
        let mut facets = Facets::default();
        if count_facets {
            let base = self.intersect(&cs, Some(Group::BoothCat));
            facets.booth = self.booth_cats.0.iter().map(|(k, b)| (k.clone(), base.intersection_len(b))).filter(|(_, n)| *n > 0).collect();
            let base = self.intersect(&cs, Some(Group::SmartCat));
            let cats = if r.scp { &self.cats_primary } else { &self.cats };
            facets.smart = cats.0.iter().map(|(k, b)| (k.clone(), base.intersection_len(b))).filter(|(_, n)| *n > 0).collect();
            let base = self.intersect(&cs, Some(Group::Features));
            facets.features = FEATURES.iter().map(|f| (f.to_string(), base.intersection_len(self.features.0.get(*f).unwrap_or(&RoaringBitmap::new())))).collect();
        }

        let asc = r.asc;
        let (sort, scores) = match (r.sort, &text) {
            (Sort::Relevant, Some(t)) => ("relevant", Some(&t.matches.scores)),
            (Sort::Newest, _) => ("newest", None),
            (Sort::Price, _) => ("price", None),
            (Sort::Likes, _) => ("likes", None),
            (Sort::Title, _) => ("title", None),
            _ => ("trending", None),
        };
        let key = |i: u32| -> f64 {
            let i = i as usize;
            match sort {
                "relevant" => scores.and_then(|s| s.get(&(i as u32))).copied().unwrap_or(0.0) as f64,
                "newest" => self.published[i] as f64,
                "price" => self.price[i].map_or(if asc { f64::INFINITY } else { f64::NEG_INFINITY }, f64::from),
                "likes" => self.likes[i] as f64,
                "title" => -(self.title_rank[i] as f64),
                _ => self.trending[i] as f64,
            }
        };
        let cmp = |a: &u32, b: &u32| {
            let o = key(*b).total_cmp(&key(*a));
            let o = if asc { o.reverse() } else { o };
            let o = if sort == "relevant" { o.then_with(|| self.trending[*b as usize].total_cmp(&self.trending[*a as usize])) } else { o };
            o.then(a.cmp(b))
        };

        let page_size = r.page_size;
        let count = hits.len();
        let total_pages = (count as usize).div_ceil(page_size).max(1);
        let page = r.page.clamp(1, total_pages);
        let start = (page - 1) * page_size;
        let end = (page * page_size).min(count as usize);
        let mut docs: Vec<u32> = hits.iter().collect();
        // Partition at both page boundaries so deep pages don't sort every preceding result.
        if end < docs.len() && end > 0 {
            docs.select_nth_unstable_by(end - 1, cmp);
            docs.truncate(end);
        }
        if start > 0 {
            docs.select_nth_unstable_by(start, cmp);
        }
        let page_docs = &mut docs[start..];
        page_docs.sort_unstable_by(cmp);
        let ids = page_docs.iter().map(|&d| self.ids[d as usize]).collect();

        Ok(SearchResult {
            count,
            page,
            total_pages,
            page_size,
            sort,
            dir: if asc { "asc" } else { "desc" },
            ids,
            facets,
            typo: text.as_ref().is_some_and(|t| t.matches.typo),
            expanded_with: text.filter(|t| !t.expansion.is_disjoint(&hits)).map(|t| t.expanded.to_vec()).unwrap_or_default(),
        })
    }
}
