use super::fusion::fuse;
use super::tests::{doc, ids, request};
use super::*;

#[test]
fn a_named_model_matches_its_items_and_keeps_full_query_hits() {
    let mut d = (1..=4).map(doc).collect::<Vec<_>>();
    d[0].title = "しなの short hair".into();
    d[0].avatars = vec!["shinano".into()];
    d[1].title = "shinano short hair".into();
    d[2].title = "long dress".into();
    d[2].avatars = vec!["shinano".into()];
    d[3].title = "short hair".into();
    d[3].avatars = vec!["manuka".into()];
    let c = Catalog::build(d, 0, None).unwrap();
    let models = vec!["shinano".to_owned()];
    let run = |q: &str, rest: &str| {
        let sem = Semantic { ids: &[], categories: &[], models: &models, rest };
        let mut v = c.search(&request(&format!("q={q}&sort=relevant")), 0, None, Some(sem), None, true).unwrap().ids;
        let first = v[0];
        v.sort();
        (first, v)
    };
    assert_eq!(run("shinano", ""), (2, vec![1, 2, 3]));
    assert_eq!(run("shinano%20short%20hair", "short hair"), (2, vec![1, 2]));
}

#[test]
fn fusion_rewards_agreement_and_keeps_semantic_only_hits() {
    let text: FxHashMap<u32, f32> = [(1, 9.0), (2, 5.0), (3, 1.0)].into_iter().collect();
    let f = fuse(&text, &[3, 7], &FxHashMap::default(), &RoaringBitmap::new());
    assert!(f[&3] > f[&1]);
    assert!(f[&7] > 0.0 && f[&7] < f[&2]);
    assert_eq!(f.len(), 4);
    let g = fuse(&text, &[7], &FxHashMap::default(), &[2, 8].into_iter().collect());
    assert!(g[&8] > g[&7] && g[&8] < g[&1]);
    assert!(g[&2] > g[&1]);
}

#[test]
fn re_uploaded_items_stay_out_of_search_but_not_favourites() {
    let mut d = (1..=3).map(doc).collect::<Vec<_>>();
    d[0].relisted = true;
    d[2].booth_category = Some(AVATAR_CATEGORY.into());
    d[2].supported = true;
    d[2].relisted = true;
    let c = Catalog::build(d, 0, None).unwrap();
    assert_eq!(ids(&c, ""), [2]);
    assert_eq!(ids(&c, "q=item"), [2]);
    assert!(ids(&c, "v=avatars").is_empty());
    let favs = c.docs_for(&[1, 3]);
    let mut f = c.search_plain("v=favourites&pageSize=10", 0, Some(&favs)).unwrap().ids;
    f.sort();
    assert_eq!(f, [1, 3]);
    assert_eq!(c.docs_for(&[1]).len(), 1);
}

#[test]
fn a_search_also_finds_what_the_tags_translating_it_find() {
    let c = crate::expansion::tests::catalogue();
    let run = |q: &str, expand| c.search(&request(&format!("{q}&pageSize=100")), 0, None, None, expand, true).unwrap();
    let (words, both) = (run("q=skirt", None), run("q=skirt", Some("skirt")));
    let mut added: Vec<i64> = both.ids.iter().filter(|id| !words.ids.contains(id)).copied().collect();
    added.sort();
    assert_eq!(added, (16..=25).chain(34..=50).collect::<Vec<_>>());
    assert_eq!((words.count, both.count), (23, 50));
    assert_eq!((words.expanded_with, both.expanded_with), (vec![], vec!["スカート".to_owned()]));
    assert_eq!(run("q=skirt&free=1", Some("skirt")).ids, [1]);
    assert!(run("q=skirt&free=1", Some("skirt")).expanded_with.is_empty());
}

#[test]
fn translated_matches_vote_once_at_half_weight() {
    let c = crate::expansion::tests::catalogue();
    let r = c.search(&request("q=tail&sort=relevant&pageSize=100"), 0, None, None, Some("tail"), true).unwrap();
    let mut first = r.ids[..12].to_vec();
    first.sort();
    assert_eq!(first, (71..=82).collect::<Vec<_>>());
    assert_eq!(r.ids.len(), 20);
}

#[test]
fn word_matches_keep_their_order_among_themselves() {
    let c = crate::expansion::tests::catalogue();
    let semantic = Semantic { ids: &[16, 51, 3, 30], categories: &[], models: &[], rest: "skirt" };
    let words: Vec<i64> = (1..=15).chain(26..=33).collect();
    let order = |expand| {
        let r = c.search(&request("q=skirt&sort=relevant&pageSize=100"), 0, None, Some(semantic), expand, true).unwrap();
        r.ids.into_iter().filter(|id| words.contains(id)).collect::<Vec<_>>()
    };
    assert_eq!(order(Some("skirt")), order(None));
}

#[test]
fn equal_scores_rank_by_trending_after_fusion() {
    let c = crate::expansion::tests::catalogue();
    let semantic = Semantic { ids: &[51, 52], categories: &[], models: &[], rest: "skirt" };
    let order = |q: &str, sem, expand, among: &[i64]| {
        let r = c.search(&request(&format!("q={q}&sort=relevant&pageSize=100")), 0, None, sem, expand, true).unwrap();
        r.ids.into_iter().filter(|id| among.contains(id)).collect::<Vec<_>>()
    };
    let words: Vec<i64> = (1..=15).chain(26..=33).collect();
    let plain = order("skirt", None, None, &words);
    assert_eq!(plain[..3], [7, 6, 5]);
    assert_eq!(order("skirt", Some(semantic), None, &words), plain);
    assert_eq!(order("skirt", None, Some("skirt"), &words), plain);
    let translated: Vec<i64> = (16..=25).chain(34..=50).collect();
    assert_eq!(order("skirt", None, Some("skirt"), &translated), order("スカート", None, None, &translated));
}

#[test]
fn with_a_model_named_only_fitting_translated_matches_join() {
    let c = crate::expansion::tests::catalogue();
    let models = vec!["shinano".to_owned()];
    let semantic = Semantic { ids: &[], categories: &[], models: &models, rest: "skirt" };
    let mut ids = c.search(&request("q=shinano%20skirt&pageSize=100"), 0, None, Some(semantic), Some("skirt"), true).unwrap().ids;
    ids.sort();
    assert_eq!(ids, [1, 2, 16, 17]);
}
