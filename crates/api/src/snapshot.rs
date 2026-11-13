//! The in-memory catalogue snapshot: built from PostgreSQL at startup, rebuilt when the worker bumps
//! catalog_meta.version, and swapped in atomically while the old one keeps serving.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::AppState;
use crate::catalog::Catalog;
use crate::db;
use crate::params::Params;
use crate::text::{TYPO_MIN, fold, fold_width, one_typo};

/// Name lookup for sidebar pickers and the command palette.
pub(crate) struct Lookup(Vec<(Vec<String>, Value, u64)>);

impl Lookup {
    /// Splits model names out of a search ("shinano short hair" -> "short hair" + [shinano]); a name is a whole
    /// word (or two) equal to a model's slug or name in any language, at least 3 characters. Each slug comes with
    /// the query words it was read from, as typed (the site marks those tokens); a name repeated in a row is one
    /// entry, with the first one's words.
    ///
    /// A word that is not a name, and not a word the catalogue knows (`known`), is also read as the one model whose
    /// Latin name it misspells by one typo, as the text search allows ("shinnano" -> shinano); the `bool` says so.
    pub(crate) fn split_names<'q>(&self, q: &'q str, known: impl Fn(&str) -> bool) -> (String, Vec<(String, Vec<&'q str>)>, bool) {
        let words: Vec<&str> = q.split_whitespace().collect();
        let slug = |e: &(Vec<String>, Value, u64)| e.1["slug"].as_str().map(str::to_owned);
        let slug_of = |w: &str| {
            let w = fold_width(w);
            (w.chars().count() >= 3).then(|| self.0.iter().find(|(keys, _, _)| keys.contains(&w))).flatten().and_then(slug)
        };
        let latin = |w: &str| w.len() >= TYPO_MIN && w.bytes().all(|b| b.is_ascii_lowercase());
        let misspelt = |w: &str| {
            let w = fold_width(w);
            if !latin(&w) || known(&w) {
                return None;
            }
            let mut near = self.0.iter().filter(|(keys, _, _)| keys.iter().any(|k| latin(k) && one_typo(k, &w))).filter_map(slug);
            let first = near.next()?;
            near.all(|s| s == first).then_some(first)
        };
        let (mut rest, mut slugs, mut typo, mut i) = (Vec::new(), Vec::new(), false, 0);
        while i < words.len() {
            if let Some(slug) = words.get(i + 1).and_then(|next| slug_of(&format!("{} {next}", words[i]))) {
                slugs.push((slug, words[i..i + 2].to_vec()));
                i += 2;
                continue;
            }
            if let Some(slug) = slug_of(words[i]) {
                slugs.push((slug, vec![words[i]]));
            } else if let Some(slug) = misspelt(words[i]) {
                // a name repeated in a row is kept as its first spelling (below): a dropped misspelling is no typo
                typo |= slugs.last().is_none_or(|(s, _)| *s != slug);
                slugs.push((slug, vec![words[i]]));
            } else {
                rest.push(words[i]);
            }
            i += 1;
        }
        slugs.dedup_by(|b, a| a.0 == b.0);
        (rest.join(" "), slugs, typo)
    }

    pub(crate) fn new(mut entries: Vec<(Vec<String>, Value, u64)>) -> Self {
        entries.retain(|e| e.2 > 0);
        for e in &mut entries {
            e.0.iter_mut().for_each(|s| *s = fold_width(s));
        }
        entries.sort_by_key(|e| std::cmp::Reverse(e.2));
        Self(entries)
    }

    pub(crate) fn find(&self, p: &Params) -> Value {
        let q = fold_width(&p.get("q").unwrap_or_default());
        let offset = p.num("offset").unwrap_or(0);
        let limit = p.num("limit").unwrap_or(20).clamp(1, 100);
        let items: Vec<Value> = self
            .0
            .iter()
            .filter(|(keys, _, _)| q.is_empty() || keys.iter().any(|k| k.contains(&q)))
            .skip(offset)
            .take(limit)
            .map(|(_, v, n)| {
                let mut v = v.clone();
                v["item_count"] = json!(n);
                v
            })
            .collect();
        json!({ "items": items })
    }
}

/// Search texts that name one smart category, without spaces and folded (`text::fold`) ("shorthair", "ショート" -> hair--short):
/// a parent's name, a child's name when no other child shares it, or child and parent names together
/// in either order, per language.
pub(crate) fn category_phrases(categories: &Value) -> HashMap<String, String> {
    let norm = |s: &str| fold(&s.split_whitespace().collect::<String>());
    let langs = ["en", "ja", "ko", "zh"];
    let parents = categories.as_array().map(Vec::as_slice).unwrap_or_default();
    let mut child_names: HashMap<String, Vec<String>> = HashMap::new();
    let mut out = HashMap::new();
    for p in parents {
        let slug = p["slug"].as_str().unwrap_or_default();
        for l in langs {
            let Some(pn) = p[l].as_str().map(norm).filter(|n| !n.is_empty()) else { continue };
            out.insert(pn.clone(), slug.to_owned());
            for c in p["children"].as_array().into_iter().flatten() {
                let (Some(cs), Some(cn)) = (c["slug"].as_str(), c[l].as_str().map(norm).filter(|n| !n.is_empty())) else { continue };
                out.insert(format!("{cn}{pn}"), cs.to_owned());
                out.insert(format!("{pn}{cn}"), cs.to_owned());
                child_names.entry(cn).or_default().push(cs.to_owned());
            }
        }
    }
    for (name, mut slugs) in child_names {
        slugs.dedup();
        if let [only] = slugs.as_slice() {
            out.entry(name).or_insert_with(|| only.clone());
        }
    }
    out
}

/// Everything derived from the catalogue; rebuilt off to the side and swapped in atomically.
pub(crate) struct Snapshot {
    pub(crate) catalog: Catalog,
    /// see `category_phrases`
    pub(crate) category_phrases: HashMap<String, String>,
    pub(crate) avatars: Lookup,
    pub(crate) authors: Lookup,
    pub(crate) tags: Lookup,
    pub(crate) booth_tags: Lookup,
    pub(crate) categories: Value,
    pub(crate) version: i64,
    pub(crate) loaded_at: chrono::DateTime<chrono::Utc>,
    pub(crate) build_secs: f64,
}

pub(crate) async fn catalog_version(pool: &PgPool) -> Result<i64> {
    Ok(sqlx::query_scalar("select version from catalog_meta").fetch_one(pool).await?)
}

/// A rebuild reloads every item (~25 s, twice the memory), so worker bumps during a busy backlog are
/// folded into at most one rebuild per gap. The admin "reload now" skips the wait.
const MIN_RELOAD_GAP: std::time::Duration = std::time::Duration::from_secs(300);

/// Rebuilds the snapshot when the worker bumps catalog_meta.version (checked every 30 s, rebuilt at most
/// once per `MIN_RELOAD_GAP`, or on demand).
/// The old snapshot keeps serving during the rebuild, so peak memory is roughly two catalogues.
pub(crate) async fn reloader(state: Arc<AppState>) {
    let mut last_build = Instant::now();
    loop {
        let forced = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => false,
            _ = state.reload.notified() => true,
        };
        if !forced && last_build.elapsed() < MIN_RELOAD_GAP {
            continue;
        }
        let version = match catalog_version(&state.pool).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("reload check: {e:#}");
                continue;
            }
        };
        if version == state.snap().version {
            continue;
        }
        last_build = Instant::now();
        match build_snapshot(&state.pool, version).await {
            Ok(s) => {
                println!("catalogue reloaded: version {version}, {} docs", s.catalog.len());
                state.snap.store(Arc::new(s));
            }
            Err(e) => eprintln!("catalogue reload failed, keeping version {}: {e:#}", state.snap().version),
        }
    }
}

pub(crate) async fn build_snapshot(pool: &PgPool, version: i64) -> Result<Snapshot> {
    let pool = pool.clone();
    let t0 = Instant::now();
    let docs = db::load_docs(&pool).await?;
    println!("loaded {} docs in {:.1}s", docs.len(), t0.elapsed().as_secs_f64());
    let t1 = Instant::now();
    // Text content selects the disk index; metadata-only reloads reuse it. INDEX_DIR="" keeps it in RAM.
    let index_root = std::env::var("INDEX_DIR").unwrap_or_else(|_| "textindex".into());
    let dir = (!index_root.is_empty()).then(|| std::path::PathBuf::from(index_root));
    let catalog = tokio::task::spawn_blocking(move || Catalog::build(docs, chrono::Utc::now().timestamp(), dir.as_deref())).await??;
    println!("built index in {:.1}s, bitmaps ~{} KB", t1.elapsed().as_secs_f64(), catalog.bitmap_bytes() / 1024);

    let avatars = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>)>("select slug, name_en, name_ja, name_ko, coalesce((select b.images[1] from booth_images b where b.item_id = a.avatar_item_id), a.image_url) from avatars a")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(slug, en, ja, ko, image_url)| {
            let n = catalog.avatars.len(&slug);
            let keys = [Some(slug.clone()), Some(en.clone()), ja.clone(), ko.clone()].into_iter().flatten().collect();
            (keys, json!({ "slug": slug, "en": en, "ja": ja, "ko": ko, "image_url": image_url }), n)
        })
        .collect();
    let authors = sqlx::query_as::<_, (String, Option<String>)>("select distinct on (author_name) author_name, author_icon from items order by author_name, scraped_at desc")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(name, icon)| (vec![name.clone()], json!({ "name": name, "image_url": icon }), catalog.authors.len(&name)))
        .collect();
    let tags = sqlx::query_as::<_, (i32, String, String, Option<String>, Vec<String>)>("select id, name, display, display_zh, aliases from smart_tags")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(id, name, display, display_zh, aliases)| {
            let n = catalog.smart_tags.len(&name);
            let keys = aliases.iter().cloned().chain([name.clone(), display.clone()]).chain(display_zh.clone()).collect();
            (keys, json!({ "name": name, "display": display, "display_zh": display_zh, "master_id": id, "aliases": aliases }), n)
        })
        .collect();
    let booth_tags = catalog.booth_tags.keys().map(|t| (vec![t.clone()], json!({ "name": t }), catalog.booth_tags.len(t))).collect();
    let categories: Value = sqlx::query_scalar(
        "select coalesce(jsonb_agg(jsonb_build_object('slug', p.slug, 'en', p.name_en, 'ja', p.name_ja, 'ko', p.name_ko, 'zh', p.name_zh, 'mode', p.mode,
            'children', (select coalesce(jsonb_agg(jsonb_build_object('slug', c.slug, 'en', c.name_en, 'ja', c.name_ja, 'ko', c.name_ko, 'zh', c.name_zh,
                'group_en', c.group_en, 'group_ja', c.group_ja, 'group_ko', c.group_ko, 'group_zh', c.group_zh) order by c.name_en), '[]')
                from smart_categories c where c.parent_slug = p.slug)) order by p.name_en), '[]')
         from smart_categories p where p.parent_slug is null",
    )
    .fetch_one(&pool)
    .await?;

    Ok(Snapshot {
        catalog,
        category_phrases: category_phrases(&categories),
        avatars: Lookup::new(avatars),
        authors: Lookup::new(authors),
        tags: Lookup::new(tags),
        booth_tags: Lookup::new(booth_tags),
        categories: json!({ "categories": categories }),
        version,
        loaded_at: chrono::Utc::now(),
        build_secs: t0.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn lookup_keeps_input_order_when_counts_tie() {
        let entries = [("first", 10), ("second", 10), ("popular", 20), ("unused", 0)]
            .into_iter().map(|(slug, count)| (vec![slug.into()], json!({ "slug": slug }), count)).collect();
        let lookup = Lookup::new(entries);
        let slugs: Vec<_> = lookup.0.iter().map(|(_, item, _)| item["slug"].as_str().unwrap()).collect();
        assert_eq!(slugs, ["popular", "first", "second"]);
    }

    #[test]
    fn model_names_are_split_out_of_the_query() {
        let entry = |slug: &str, names: &[&str]| {
            (names.iter().map(|n| n.to_string()).chain([slug.to_string()]).collect(), json!({ "slug": slug }), 10u64)
        };
        let l = Lookup::new(vec![entry("shinano", &["Shinano", "しなの"]), entry("rusk", &["Rusk Dragon"]), entry("io", &["IO"]), entry("kokoa", &["ここあ"]), entry("lime", &["ＬＩＭＥ Ｄｏｌｌ"])]);
        let split = |q| {
            let (rest, names, _) = l.split_names(q, |_| false);
            (rest, names.into_iter().map(|(slug, _)| slug).collect::<Vec<String>>())
        };
        assert_eq!(split("Shinano short hair"), ("short hair".into(), vec!["shinano".into()]));
        assert_eq!(split("しなの ボブ"), ("ボブ".into(), vec!["shinano".into()]));
        assert_eq!(split("rusk dragon wings"), ("wings".into(), vec!["rusk".into()]));
        // width and case do not matter; kana are kept as written, so the word ココア is not the model ここあ
        assert_eq!(split("ココア ドレス"), ("ココア ドレス".into(), vec![]));
        assert_eq!(split("ＳＨＩＮＡＮＯ bob"), ("bob".into(), vec!["shinano".into()]));
        assert_eq!(split("lime doll coat"), ("coat".into(), vec!["lime".into()]));
        // the pickers filter the same way: "ＳＨＩ" finds Shinano
        let found = l.find(&Params::parse("q=%EF%BC%B3%EF%BC%A8%EF%BC%A9"));
        assert_eq!(found["items"][0]["slug"], "shinano");
        // too short to be taken as a name
        assert_eq!(split("io hair"), ("io hair".into(), vec![]));
        assert_eq!(split("short hair"), ("short hair".into(), vec![]));
        // a name repeated in a row is one
        assert_eq!(split("shinano Shinano bob"), ("bob".into(), vec!["shinano".into()]));

        // each name with the words it was read from, as typed; a repeated one with the first one's
        let words = |q| l.split_names(q, |_| false).1;
        assert_eq!(words("Shinano short hair"), vec![("shinano".into(), vec!["Shinano"])]);
        assert_eq!(words("しなの ボブ"), vec![("shinano".into(), vec!["しなの"])]);
        assert_eq!(words("Rusk Dragon wings"), vec![("rusk".into(), vec!["Rusk", "Dragon"])]);
        assert_eq!(words("shinano Shinano bob"), vec![("shinano".into(), vec!["shinano"])]);
    }

    #[test]
    fn a_misspelt_model_name_is_read_as_the_model() {
        let entry = |slug: &str, names: &[&str]| {
            (names.iter().map(|n| n.to_string()).chain([slug.to_string()]).collect(), json!({ "slug": slug }), 10u64)
        };
        let l = Lookup::new(vec![entry("shinano", &["Shinano"]), entry("manuka", &["Manuka"]), entry("karin", &["Karin"]), entry("karen", &["Karen"]), entry("lime", &["Lime"])]);
        let split = |q, known: &[&str]| {
            let (rest, names, typo) = l.split_names(q, |w| known.contains(&w));
            (rest, names, typo)
        };
        // a letter added, swapped, changed; the words as typed
        assert_eq!(split("shinnano short hair", &[]), ("short hair".into(), vec![("shinano".into(), vec!["shinnano"])], true));
        assert_eq!(split("Mnauka dress", &[]), ("dress".into(), vec![("manuka".into(), vec!["Mnauka"])], true));
        assert!(split("shinaxo", &[]).2);
        // a name spelt right is no typo
        assert!(!split("shinano bob", &[]).2);
        // a word the catalogue has is that word, not a model one typo away
        assert_eq!(split("shinnano bob", &["shinnano"]), ("shinnano bob".into(), vec![], false));
        // two models one typo away: neither; under 5 letters or two typos: not a name
        assert_eq!(split("karon", &[]), ("karon".into(), vec![], false));
        assert_eq!(split("lume", &[]), ("lume".into(), vec![], false));
        assert_eq!(split("shinnanoo", &[]), ("shinnanoo".into(), vec![], false));
        // a misspelling after the right name is dropped with its repeat, so no typo was read
        assert_eq!(split("shinano shinnano bob", &[]), ("bob".into(), vec![("shinano".into(), vec!["shinano"])], false));
        assert!(split("shinnano shinano bob", &[]).2);
    }
}
