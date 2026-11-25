use super::*;
use crate::params::Params;
use crate::tag_words::TagWords;

pub(super) fn doc(id: i64) -> DocInput {
    DocInput {
        id,
        title: format!("item {id}"),
        titles_translated: vec![],
        title_zh: None,
        title_is_zh: false,
        title_languages: 0b0111,
        bio: String::new(),
        author: "a".into(),
        booth_category: Some("3D Clothing".into()),
        price: Some(1000),
        likes: 0,
        published: Some(0),
        is_adult: false,
        rating: Some("SFW".into()),
        features: vec![],
        avatars: vec![],
        categories: vec![],
        booth_tags: vec![],
        smart_tags: vec![],
        supported: false,
        gender: None,
        relisted: false,
    }
}

pub(super) fn request(query: &str) -> SearchRequest {
    SearchRequest::from(&Params::parse(query))
}

pub(super) fn dictionary(rows: &[(&str, &[&str])]) -> TagWords {
    TagWords::new(rows.iter().map(|(tag, words)| (tag.to_string(), words.iter().map(|w| w.to_string()).collect())))
}

impl Catalog {
    pub(super) fn search_plain(&self, query: &str, now: i64, favourites: Option<&RoaringBitmap>) -> Result<SearchResult> {
        self.search(&request(query), now, favourites, None, None, true)
    }
}

pub(super) fn ids(c: &Catalog, q: &str) -> Vec<i64> {
    let mut v = c.search_plain(&format!("{q}&pageSize=100"), 0, None).unwrap().ids;
    v.sort();
    v
}

fn catalog() -> Catalog {
    let mut d = (1..=6).map(doc).collect::<Vec<_>>();
    d[0].avatars = vec!["manuka".into()];
    d[0].features = vec!["ma".into(), "liltoon".into()];
    d[0].categories = vec![("clothing".into(), true), ("clothing--bikinis".into(), true)];
    d[1].avatars = vec!["shinano".into()];
    d[1].features = vec!["ma".into()];
    d[1].categories = vec![("clothing".into(), false)];
    d[1].price = Some(0);
    d[2].is_adult = true;
    d[3].rating = Some("Suggestive".into());
    d[4].rating = Some("NSFW".into());
    d[5].booth_category = Some("3D Characters".into());
    d[5].supported = true;
    d[5].gender = Some("female".into());
    Catalog::build(d, 0, None).unwrap()
}

#[test]
fn an_item_named_by_its_link_or_number_ranks_first_among_the_hits() {
    let mut d = (10_001..=10_005).map(doc).collect::<Vec<_>>();
    for (doc, title) in d.iter_mut().zip(["Ribbon", "Set for 10001", "Old set for 10001", "Adult ribbon", "Avatar"]) {
        doc.title = title.into();
    }
    d[0].relisted = true;
    d[2].relisted = true;
    d[3].is_adult = true;
    d[4].booth_category = Some(AVATAR_CATEGORY.into());
    let c = Catalog::build(d, 0, None).unwrap();
    let found = |q: &str| c.search_plain(q, 0, None).unwrap().ids;
    assert_eq!(found("q=10001"), [10_001, 10_002]);
    assert_eq!(found("q=https://booth.pm/ja/items/10001"), [10_001]);
    assert_eq!((found("q=10005"), found("q=10005&v=avatars")), (vec![10_005], vec![10_005]));
    assert_eq!(c.search_plain("q=10001&v=favourites", 0, Some(&RoaringBitmap::new())).unwrap().ids, [10_001]);
    assert_eq!((c.referenced("booth.pm/ja/items/10009"), c.referenced("10009")), (Some(None), None));
    assert!(found("q=10004").is_empty());
    assert_eq!(found("q=10004&adult=include"), [10_004]);
    let q = "q=10001&free=1&without=free";
    let (r, wider) = crate::search::search_and_wider(&c, q, &request(q), 0, None, None, None).unwrap();
    assert_eq!((r.count, wider), (0, vec![2]));
}

#[test]
fn adult_modes_match_vrcatalogue_rules() {
    let c = catalog();
    assert_eq!(ids(&c, ""), [1, 2, 4]);
    assert_eq!(ids(&c, "adult=adult_only"), [3, 5]);
    assert_eq!(ids(&c, "adult=suggestive"), [4]);
    assert_eq!(ids(&c, "adult=include"), [1, 2, 3, 4, 5]);
}

#[test]
fn group_semantics() {
    let c = catalog();
    assert_eq!(ids(&c, "avatars=manuka+shinano"), [1, 2]);
    assert_eq!(ids(&c, "if=ma+liltoon"), [1]);
    assert_eq!(ids(&c, "sc=clothing--bikinis"), [1]);
    assert_eq!(ids(&c, "sc=clothing&scp=1"), [1]);
    assert_eq!(ids(&c, "free=1"), [2]);
    assert_eq!(ids(&c, "v=avatars&gender=female"), [6]);
    assert_eq!(ids(&c, "v=avatars&asup=unsupported"), Vec::<i64>::new());
}

#[test]
fn a_search_says_when_its_words_matched_only_with_a_typo() {
    let mut ribbon = doc(1);
    ribbon.title = "Shinano Ribbon".into();
    let c = Catalog::build(vec![ribbon, doc(2)], 0, None).unwrap();
    let typo = |q: &str| c.search_plain(q, 0, None).unwrap().typo;
    assert!(typo("q=shinnano"));
    assert!(!typo("q=shinano") && !typo("q=xyzzyq") && !typo("free=1"));
}

#[test]
fn an_empty_search_counts_what_each_without_group_finds() {
    let c = catalog();
    let wider = |q: &str| {
        let (r, counts) = crate::search::search_and_wider(&c, q, &request(q), 0, None, None, None).unwrap();
        (r.count, counts)
    };
    assert_eq!(wider("avatars=manuka&free=1&if=liltoon&without=avatars+free+if+nonsense"), (0, vec![0, 1, 0, 0]));
    assert_eq!(wider("avatars=manuka&pmin=1&pmax=500&without=pmin,pmax+avatars"), (0, vec![1, 0]));
    assert_eq!(wider("q=item%201&free=1&without=free+q"), (0, vec![1, 0]));
    assert_eq!(wider("avatars=manuka&without=avatars"), (1, vec![]));
    assert_eq!(wider("avatars=manuka&free=1"), (0, vec![]));
}

#[test]
fn facets_ignore_own_group_and_skipping_facets_preserves_results() {
    let c = catalog();
    let r = c.search_plain("if=liltoon", 0, None).unwrap();
    assert_eq!(r.count, 1);
    assert_eq!(r.facets.features["ma"], 2);
    assert_eq!(r.facets.smart["clothing"], 1);

    let favourites = c.docs_for(&[1, 2, 4, 6]);
    let models = vec!["manuka".to_owned()];
    let categories = vec!["clothing".to_owned()];
    let semantic = Semantic { ids: &[4, 2], categories: &categories, models: &models, rest: "item" };
    for query in [
        "", "q=item", "q=item&pageSize=2&page=2", "q=missing", "if=liltoon", "sc=clothing&scp=1",
        "adult=include&sort=price&dir=asc", "v=avatars", "v=favourites", "free=1", "avatars=manuka",
    ] {
        let p = request(query);
        for sem in [None, Some(semantic)] {
            let mut full = c.search(&p, 0, Some(&favourites), sem, None, true).unwrap();
            let light = c.search(&p, 0, Some(&favourites), sem, None, false).unwrap();
            assert_eq!(light.facets, Facets::default());
            full.facets = Facets::default();
            assert_eq!(full, light, "query {query:?}");
        }
    }
}

#[test]
fn sort_and_paging_match_full_sort_and_favourites_span_all_kinds() {
    let c = catalog();
    let r = c.search_plain("adult=include&sort=price&dir=asc&pageSize=2&page=1", 0, None).unwrap();
    assert_eq!(r.ids, [2, 1]);
    assert_eq!(r.total_pages, 3);
    assert_eq!(c.search_plain("adult=include&page=99", 0, None).unwrap().page, 1);

    let favs = c.docs_for(&[1, 6, 999]);
    let mut fav_ids = c.search_plain("v=favourites&adult=include", 0, Some(&favs)).unwrap().ids;
    fav_ids.sort();
    assert_eq!(fav_ids, [1, 6]);

    let docs = (1..=73)
        .map(|id| {
            let mut d = doc(id);
            d.title = format!("item {}", id % 11);
            d.price = (id % 7 != 0).then_some((id % 9) as i32 * 100);
            d.likes = (id % 5) as i32;
            d.published = (id % 6 != 0).then_some(id % 13 * 86_400);
            d
        })
        .collect();
    let c = Catalog::build(docs, 2_000_000, None).unwrap();
    for query in ["", "q=item", "free=1", "q=missing"] {
        for sort in ["trending", "newest", "price", "likes", "title", "relevant"] {
            for dir in ["asc", "desc"] {
                let base = format!("{query}&sort={sort}&dir={dir}");
                let full = c.search(&request(&format!("{base}&pageSize=100")), 0, None, None, None, false).unwrap();
                for size in [1, 7, 20, 100] {
                    let pages = full.ids.len().div_ceil(size).max(1);
                    for page in [0, 1, 2, 5, pages, pages + 1, usize::MAX] {
                        let params = request(&format!("{base}&pageSize={size}&page={page}"));
                        let r = c.search(&params, 0, None, None, None, false).unwrap();
                        let page = page.clamp(1, pages);
                        let expected: Vec<_> = full.ids.iter().skip((page - 1) * size).take(size).copied().collect();
                        assert_eq!(r.ids, expected, "{base}, size {size}, page {page}");
                        assert_eq!((r.count, r.page, r.total_pages), (full.count, page, pages));
                    }
                }
            }
        }
    }
}
