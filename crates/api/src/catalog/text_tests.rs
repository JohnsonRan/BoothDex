use super::tests::{dictionary, doc, ids, request};
use super::*;
use crate::params::Params;

#[test]
fn metadata_reload_reuses_text_index_and_preserves_complete_results() {
    let root = std::env::temp_dir().join(format!("catalog-reuse-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let inputs = |changed: bool| {
        (1..=32)
            .map(|id| {
                let mut d = doc(id);
                d.title = if id % 2 == 0 { "Short Hair".into() } else { "ゴシックドレス".into() };
                d.titles_translated = vec!["衣装 ヘア".into()];
                d.avatars = vec!["shinano".into()];
                d.categories = vec![("clothing".into(), true)];
                d.likes = if changed { 100 - id as i32 } else { id as i32 };
                d.price = Some(if changed && id % 3 == 0 { 0 } else { 1000 });
                d.is_adult = changed && id % 5 == 0;
                d.features = if changed && id % 4 == 0 { vec!["ma".into()] } else { vec![] };
                d
            })
            .collect()
    };
    let original = Catalog::build(inputs(false), 0, Some(&root)).unwrap();
    let index_dir = std::fs::read_dir(&root).unwrap().next().unwrap().unwrap().path();
    let sentinel = index_dir.join("reused");
    std::fs::write(&sentinel, "keep").unwrap();
    let cached = Catalog::build(inputs(true), 0, Some(&root)).unwrap();
    let rebuilt = Catalog::build(inputs(true), 0, None).unwrap();
    assert!(sentinel.exists(), "metadata changes must not rebuild the text index");
    let models = vec!["shinano".to_owned()];
    let categories = vec!["clothing".to_owned()];
    let semantic = Semantic { ids: &[32, 8, 2], categories: &categories, models: &models, rest: "hair" };
    let favourites = cached.docs_for(&[2, 3, 4, 5, 6, 8, 32]);
    for query in [
        "", "q=hair", "q=ヘア", "q=ゴシック", "q=hair&pageSize=3&page=2",
        "q=hair&sort=likes", "q=hair&sort=price&dir=asc", "q=hair&sort=title",
        "q=hair&adult=include&if=ma", "q=hair&free=1", "q=hair&sc=clothing", "q=hair&v=favourites",
    ] {
        let p = request(query);
        for sem in [None, Some(semantic)] {
            assert_eq!(cached.search(&p, 0, Some(&favourites), sem, None, true).unwrap(), rebuilt.search(&p, 0, Some(&favourites), sem, None, true).unwrap(), "query {query:?}");
        }
    }
    assert_ne!(original.search_plain("free=1", 0, None).unwrap().count, cached.search_plain("free=1", 0, None).unwrap().count);
    drop((original, cached, rebuilt));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn text_index_identity_includes_item_ids_and_text_changes() {
    let root = std::env::temp_dir().join(format!("catalog-identity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut last = None;
    for (id, title) in [(1, "Hair"), (2, "Hair"), (2, "Dress")] {
        let mut d = doc(id);
        d.title = title.into();
        let c = Catalog::build(vec![d], 0, Some(&root)).unwrap();
        let current = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).find(|p| p.join("READY").exists()).unwrap();
        assert_ne!(last.as_ref(), Some(&current));
        assert_eq!(c.search_plain(&format!("q={title}"), 0, None).unwrap().ids, [id]);
        last = Some(current);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tag_translations_match_as_words_of_the_tags_only() {
    let mut d = (1..=4).map(doc).collect::<Vec<_>>();
    for (doc, tag) in d.iter_mut().zip(["スカート", "椅子", "Hair Clip", "テクスチャ"]) {
        doc.booth_tags = vec![tag.into()];
    }
    let words = dictionary(&[("スカート", &["skirt", "裙子"]), ("椅子", &["chair", "의자"]), ("テクスチャ", &["texture", "纹理", "贴图"])]);
    let c = Catalog::build_with_tag_words(d, &words, 0, None).unwrap();
    for (q, expected) in [("skirt", vec![1]), ("裙子", vec![1]), ("chair", vec![2]), ("hair", vec![3]), ("贴图", vec![4]), ("纹理", vec![4])] {
        assert_eq!(ids(&c, &format!("q={q}")), expected, "query {q:?}");
    }
    let mut tags = c.booth_tags.keys().cloned().collect::<Vec<_>>();
    tags.sort();
    assert_eq!(tags, ["Hair Clip", "スカート", "テクスチャ", "椅子"]);
    assert!(ids(&c, "btags=skirt").is_empty());
}

#[test]
fn chinese_titles_are_searched_as_chinese() {
    let mut d = (1..=4).map(doc).collect::<Vec<_>>();
    (d[0].title, d[0].title_zh) = ("水シェーダー".into(), Some("水着色器".into()));
    (d[1].title, d[1].title_zh, d[1].title_is_zh) = ("定制服务".into(), Some("定制服务".into()), true);
    (d[2].title, d[2].title_zh) = ("スク水".into(), Some("夏日水着".into()));
    (d[3].title, d[3].title_zh) = ("学生服".into(), Some("学生制服套装".into()));
    let c = Catalog::build(d, 0, None).unwrap();
    for (q, expected) in [("水着", vec![3]), ("制服", vec![4]), ("着色器", vec![1]), ("定制", vec![2]), ("水シェーダー", vec![1])] {
        assert_eq!(ids(&c, &format!("q={q}")), expected, "query {q:?}");
        assert_eq!(c.title_hits(q).unwrap().iter().map(|d| d as i64 + 1).collect::<Vec<_>>(), expected, "title words {q:?}");
    }
}

#[test]
fn dictionary_and_chinese_title_changes_rebuild_the_text_index_on_disk() {
    let root = std::env::temp_dir().join(format!("catalog-tag-words-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let docs = || {
        let mut d = doc(1);
        d.booth_tags = vec!["スカート".into()];
        vec![d]
    };
    let index_dirs = || std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("READY").exists()).collect::<Vec<_>>();
    let first = Catalog::build_with_tag_words(docs(), &dictionary(&[("スカート", &["skirt"])]), 0, Some(&root)).unwrap();
    assert_eq!(first.search_plain("q=skirt", 0, None).unwrap().ids, [1]);
    let first_dir = index_dirs();
    drop(first);
    let second = Catalog::build_with_tag_words(docs(), &dictionary(&[("スカート", &["jupe"])]), 0, Some(&root)).unwrap();
    assert_eq!((second.search_plain("q=jupe", 0, None).unwrap().ids, second.search_plain("q=skirt", 0, None).unwrap().ids), (vec![1], vec![]));
    assert_ne!(index_dirs(), first_dir);
    drop(second);
    let _ = std::fs::remove_dir_all(&root);

    let root = std::env::temp_dir().join(format!("catalog-title-zh-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let zh_docs = |zh: &str, is_zh: bool| {
        let mut d = doc(1);
        (d.title, d.title_zh, d.title_is_zh) = (zh.into(), Some(zh.into()), is_zh);
        vec![d]
    };
    drop(Catalog::build(zh_docs("裙子", false), 0, Some(&root)).unwrap());
    let changed = Catalog::build(zh_docs("帽子", false), 0, Some(&root)).unwrap();
    assert_eq!((ids(&changed, "q=帽子"), ids(&changed, "q=裙子")), (vec![1], vec![]));
    drop(changed);
    drop(Catalog::build(zh_docs("定制服务", true), 0, Some(&root)).unwrap());
    let changed = Catalog::build(zh_docs("定制服务", false), 0, Some(&root)).unwrap();
    assert_eq!(ids(&changed, "q=制服"), [1]);
    drop(changed);
    let _ = std::fs::remove_dir_all(root);
}

/// Fixed samples of what a search finds, so a change to the text search, the tag dictionary's words or the
/// expansion shows up here (owner ruling 30, 2026-11-21).
#[test]
fn search_quality_samples() {
    let titled = |d: &mut DocInput, title: &str, translated: &str| {
        d.title = title.into();
        d.titles_translated = vec![translated.into()];
    };
    let mut docs: Vec<DocInput> = (1..=60).chain([8_962_830]).map(doc).collect();
    for d in &mut docs {
        let id = d.id;
        match id {
            1..=12 => (titled(d, &format!("スカート {id}"), &format!("Skirt {id}")), d.booth_tags = vec!["スカート".into()]).1,
            13..=22 => d.booth_tags = vec!["スカート".into()],
            23..=34 => (titled(d, &format!("尾 {id}"), &format!("Tail {id}")), d.booth_tags = vec!["尾".into()]).1,
            35..=44 => d.booth_tags = vec!["尾".into()],
            45 => d.booth_tags = vec!["椅子".into()],
            46 => d.booth_tags = vec!["カクテル".into()],
            47 => d.title = "Hair Clip".into(),
            48 => d.booth_tags = vec!["テクスチャ".into()],
            49 => (d.title, d.title_zh) = ("Water Shader".into(), Some("aleph 9 水着色器".into())),
            50 => d.title = "水着セット".into(),
            51 => (d.title, d.title_zh) = ("Fan-made Outfit".into(), Some("粉丝自制服装".into())),
            52 => (d.title, d.title_zh) = ("Commission".into(), Some("定制服务（委托）".into())),
            53 => d.title = "制服セット".into(),
            54 => d.title = "Ribbon for 8962830".into(),
            _ => {}
        }
    }
    let words = dictionary(&[
        ("スカート", &["スカート", "스커트", "裙子"]),
        ("椅子", &["chair", "椅子", "의자", "椅子"]),
        ("カクテル", &["cocktail", "カクテル", "칵테일", "鸡尾酒"]),
        ("テクスチャ", &["texture", "テクスチャ", "텍스처", "纹理", "贴图"]),
    ]);
    let c = Catalog::build_with_tag_words(docs, &words, 0, None).unwrap();
    let found = |q: &str| {
        let r = SearchRequest::from(&Params::parse(&format!("q={q}&pageSize=100")));
        let mut ids = c.search(&r, 0, None, None, Some(q), false).unwrap().ids;
        ids.sort();
        ids
    };
    let range = |r: std::ops::RangeInclusive<i64>| r.collect::<Vec<_>>();
    assert_eq!(found("skirt"), range(1..=22));
    assert_eq!(found("裙子"), range(1..=22));
    assert_eq!(found("hair"), [47]);
    assert_eq!(found("cocktail"), [46]);
    assert_eq!(found("tail"), range(23..=34));
    assert!(c.equivalents("tail").unwrap().is_empty());
    assert_eq!(found("贴图"), [48]);
    assert_eq!(found("水着"), [50]);
    assert_eq!(found("制服"), [53]);
    let first = |q: &str| c.search(&SearchRequest::from(&Params::parse(&format!("q={q}"))), 0, None, None, None, false).unwrap().ids;
    assert_eq!(first("https%3A%2F%2Fbooth.pm%2Fja%2Fitems%2F8962830"), [8_962_830]);
    assert_eq!(first("8962830"), [8_962_830, 54]);
}
