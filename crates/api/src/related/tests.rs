use super::*;
use crate::catalog::DocInput;
use crate::related::centroids::{REBUILD_AFTER, Sums};
use crate::tag_words::TagRow;

/// Groups of docs, each `(count, title, tags, shops)`, then 400 untagged docs, so that a tag on 20 items is not on
/// 5% of them. Doc ids count from 1 in order.
pub(super) fn catalogue(groups: &[(usize, &str, &[&str], usize)], each: impl Fn(i64, &mut DocInput)) -> Catalog {
    catalogue_with_words(groups, each, &crate::tag_words::TagWords::default())
}

/// `catalogue`, its text search matching the tags' translations in `words` too.
pub(super) fn catalogue_with_words(groups: &[(usize, &str, &[&str], usize)], each: impl Fn(i64, &mut DocInput), words: &crate::tag_words::TagWords) -> Catalog {
    let filler: &[(usize, &str, &[&str], usize)] = &[(400, "Filler", &[], 50)];
    let mut docs = Vec::new();
    for &(count, title, tags, shops) in groups.iter().chain(filler) {
        for i in 0..count {
            let id = docs.len() as i64 + 1;
            let mut d = DocInput {
                id,
                title: format!("{title} {id}"),
                titles_translated: vec![],
                title_zh: None,
                title_is_zh: false,
                title_languages: 0b0111,
                bio: String::new(),
                author: format!("{title} {}", i % shops),
                booth_category: Some("3D Clothing".into()),
                price: Some(1000),
                likes: 0,
                published: Some(0),
                is_adult: false,
                rating: Some("SFW".into()),
                features: vec![],
                avatars: vec![],
                categories: vec![],
                booth_tags: tags.iter().map(|t| t.to_string()).collect(),
                smart_tags: vec![],
                supported: false,
                gender: None,
                relisted: false,
            };
            each(id, &mut d);
            docs.push(d);
        }
    }
    Catalog::build_with_tag_words(docs, words, 0, None).unwrap()
}

pub(super) fn unit(i: usize) -> Vec<f32> {
    let mut v = vec![0.0; DIMS];
    v[i] = 1.0;
    v
}

/// Centroids nearest the query vector, `unit(0)`, in the order of `tags`.
pub(super) fn centroids(tags: &[&str]) -> TagCentroids {
    let mut vectors = Vec::new();
    for i in 0..tags.len() {
        let near = 0.9 - 0.01 * i as f32;
        let mut v = unit(0);
        (v[0], v[i + 1]) = (near, (1.0 - near * near).sqrt());
        vectors.extend(v);
    }
    TagCentroids { model: "m".into(), built: chrono::Utc::now(), keys: tags.iter().map(|t| fold(t)).collect(), vectors }
}

pub(super) fn related(c: &Catalog, tags: &[&str], query: &str, named: &[(String, Vec<&str>)], favourites: Option<&RoaringBitmap>) -> Vec<(String, String)> {
    related_in(c, tags, query, named, favourites, TagLabels::default(), None)
}

pub(super) fn related_in(
    c: &Catalog,
    tags: &[&str],
    query: &str,
    named: &[(String, Vec<&str>)],
    favourites: Option<&RoaringBitmap>,
    labels: TagLabels,
    language: Option<&str>,
) -> Vec<(String, String)> {
    let req = SearchRequest::from(&Params::parse(query));
    let words: Vec<&str> = named.iter().flat_map(|(_, w)| w.iter().copied()).collect();
    let meaning: String = req.q.as_deref().unwrap().split_whitespace().filter(|w| !words.contains(w)).collect::<Vec<_>>().join(" ");
    let language = language.and_then(|l| LANGUAGES.iter().position(|x| *x == l));
    let asked = Asked { req: &req, meaning: &meaning, named, favourites, now: 0, language };
    let related = Related { avatars: AvatarNames::new(["Kikyo", "桔梗", "io", "Yuri"]), centroids: None, labels };
    choose(c, &related, &centroids(tags), &unit(0), &asked).unwrap().into_iter().map(|s| (s.label, s.q)).collect()
}

pub(super) fn row(tag: &str, values: [Option<&str>; 4], keywords: &[&str]) -> TagRow {
    TagRow { tag: tag.into(), values: values.map(|v| v.map(str::to_owned)), keywords: keywords.iter().map(|k| k.to_string()).collect() }
}

pub(super) fn labels(searches: Vec<(String, String)>) -> Vec<String> {
    searches.into_iter().map(|(label, _)| label).collect()
}

const RULES: &[(usize, &str, &[&str], usize)] = &[
    (20, "Skirts", &["skirts"], 5),
    (20, "Skirt", &["spam"], 1),
    (40, "Skirt", &["broad"], 8),
    (20, "Gothic", &["gothic punk"], 5),
    (20, "Skirt", &["adult set"], 5),
    (20, "Long Skirt", &["long skirt", "ロングスカート"], 5),
    (20, "Skirt", &["long skirts"], 5),
    (20, "Skirt", &["Kikyo (桔梗)"], 5),
    (20, "Skirt", &["pleats"], 5),
    (20, "Skirt", &["frills"], 5),
    (20, "Skirt", &["lace"], 5),
    (20, "Skirt", &["denim"], 5),
    (5, "Rare", &[], 5),
];
const ORDER: &[&str] =
    &["skirts", "spam", "broad", "gothic punk", "adult set", "long skirt", "long skirts", "ロングスカート", "Kikyo (桔梗)", "pleats", "frills", "lace", "denim"];

fn rules() -> Catalog {
    catalogue(RULES, |id, d| {
        d.is_adult = (101..=120).contains(&id);
        if (121..=140).contains(&id) {
            d.avatars = vec!["shinano".into()];
            d.titles_translated = vec![format!("ロングスカート {id}")];
        }
        if (201..=220).contains(&id) {
            d.price = Some(0);
        }
    })
}

#[test]
fn each_rule_drops_its_tag_and_four_are_offered() {
    let c = rules();
    assert_eq!(labels(related(&c, ORDER, "q=skirt", &[], None)), ["long skirt", "pleats", "frills", "lace"]);
    assert_eq!(labels(related(&c, ORDER, "q=skirt&adult=include", &[], None)), ["adult set", "long skirt", "pleats", "frills"]);
    assert_eq!(labels(related(&c, ORDER, "q=skirt&free=1", &[], None)), ["frills"]);
    let favourites: RoaringBitmap = (241..=245).collect();
    assert_eq!(labels(related(&c, ORDER, "q=skirt&v=favourites", &[], Some(&favourites))), ["denim"]);
}

#[test]
fn a_named_model_stays_in_the_query_and_narrows_the_destination() {
    let c = rules();
    let named = vec![("shinano".to_owned(), vec!["Shinano"])];
    assert_eq!(related(&c, ORDER, "q=Shinano%20skirt", &named, None), [("long skirt".to_owned(), "Shinano long skirt".to_owned())]);
}

#[test]
fn without_enough_matches_the_query_does_not_judge_which_tags_go_with_it() {
    let c = rules();
    assert_eq!(labels(related(&c, &["gothic punk"], "q=rare", &[], None)), ["gothic punk"]);
    assert!(related(&c, &["gothic punk"], "q=skirt", &[], None).is_empty());
}

#[test]
fn at_most_twelve_candidates_are_checked_against_the_catalogue() {
    let names: Vec<[String; 1]> = (0..12).map(|i| [format!("adult {i}")]).collect();
    let tags: Vec<[&str; 1]> = names.iter().map(|[n]| [n.as_str()]).collect();
    let mut groups: Vec<(usize, &str, &[&str], usize)> = tags.iter().map(|t| (20, "Skirt", &t[..], 5)).collect();
    groups.push((20, "Skirt", &["pleats"], 5));
    let c = catalogue(&groups, |id, d| d.is_adult = id <= 240);
    let mut order: Vec<&str> = tags.iter().map(|[t]| *t).collect();
    order.push("pleats");
    assert!(related(&c, &order, "q=skirt", &[], None).is_empty());
    assert_eq!(labels(related(&c, &order, "q=skirt&adult=include", &[], None)), ["adult 0", "adult 1", "adult 2", "adult 3"]);
}

#[test]
fn a_tag_translating_the_query_is_not_offered_but_a_narrower_one_is() {
    let c = catalogue(&[(12, "スカート", &["スカート"], 4), (10, "Item", &["スカート"], 5), (20, "Skirt", &["ミニスカート"], 5)], |id, d| {
        if id <= 12 {
            d.titles_translated = vec![format!("Skirt {id}")];
        }
    });
    assert_eq!(c.equivalents("skirt").unwrap().to_vec(), ["スカート"]);
    assert_eq!(labels(related(&c, &["スカート", "ミニスカート"], "q=skirt", &[], None)), ["ミニスカート"]);
}

#[test]
fn a_destination_may_find_its_items_through_the_tags_translating_it() {
    let c = catalogue(&[(12, "スカート", &["スカート", "Skirt"], 4), (8, "Set", &["Skirt"], 4), (10, "Item", &["スカート"], 5)], |id, d| {
        if id <= 12 {
            d.titles_translated = vec![format!("Skirt {id}")];
        }
        if (21..=30).contains(&id) {
            d.booth_category = Some("3D Accessory".into());
        }
    });
    assert_eq!(c.equivalents("Skirt").unwrap().to_vec(), ["スカート"]);
    assert_eq!(labels(related(&c, &["Skirt"], "q=bottoms&bc=3D%20Accessory", &[], None)), ["Skirt"]);
    assert!(related(&c, &["Skirt"], "q=bottoms&bc=3D%20Avatar", &[], None).is_empty());
}

#[test]
fn a_centroid_is_the_mean_direction_of_enough_embedded_items_and_ranks_by_cosine() {
    let c = catalogue(&[(20, "A", &["a"], 5), (20, "B", &["b"], 5), (1, "Old", &["a"], 1)], |id, d| d.relisted = id == 41);
    let mut sums = Sums::new(&c, [1, 2, 3, 4, 5, 21, 22, 23, 24, 41, 999]);
    let scaled = |i, by: f32| unit(i).into_iter().map(move |x| x * by);
    for id in [1, 2, 3] {
        sums.add(id, scaled(1, 2.0));
    }
    for id in [4, 5] {
        sums.add(id, scaled(2, 5.0));
    }
    for id in [21, 22, 23, 24] {
        sums.add(id, scaled(1, 1.0));
    }
    sums.add(41, scaled(3, 1.0));
    sums.add(999, scaled(3, 1.0));
    let t = sums.finish("m".into(), chrono::Utc::now());
    assert_eq!(t.keys, ["a"]);
    let (x, y) = (3.0 / 13f32.sqrt(), 2.0 / 13f32.sqrt());
    assert!((t.vectors[1] - x).abs() < 1e-6 && (t.vectors[2] - y).abs() < 1e-6 && t.vectors[3] == 0.0);
    assert_eq!(t.vectors.len(), DIMS);

    let ranked = TagCentroids { model: "m".into(), built: chrono::Utc::now(), keys: vec!["c".into(), "b".into(), "a".into()], vectors: [unit(2), unit(1), unit(2)].concat() };
    let query: Vec<f32> = unit(2).iter().map(|x| x * 3.0).collect();
    assert_eq!(ranked.ranked(&query).collect::<Vec<_>>(), ["a", "c", "b"]);
}

/// A snapshot searches the tag dictionary's words and labels related searches with its values.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn a_snapshot_reads_the_tag_dictionary(pool: sqlx::PgPool) {
    sqlx::raw_sql(
        "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, booth_tags)
         select i, 'Item ' || i, 'creator', 'https://example.test', false, 'ok', now(), '{テクスチャ}' from generate_series(1, 20) i;
         insert into booth_tag_translations (tag, en, ja, ko, zh, keywords, source) values
         ('テクスチャ', 'texture', 'テクスチャ', null, '纹理', '{}', 'import');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let snap = crate::snapshot::build_snapshot(&pool, 1, None, None).await.unwrap();
    let texture = SearchRequest::from(&Params::parse("q=texture"));
    assert_eq!(snap.catalog.search(&texture, 0, None, None, None, false).unwrap().count, 20);
    assert_eq!(snap.related.labels.label("テクスチャ", 3), Some("纹理"));
}

/// Non-empty centroids carry over while recent and from the current model; empty ones always rebuild.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn centroids_carry_over_until_the_model_changes_or_they_age(pool: sqlx::PgPool) {
    let c = catalogue(&[(20, "A", &["a"], 5)], |_, _| {});
    assert!(TagCentroids::current(&pool, &c, None).await.unwrap().is_none(), "no item embedded yet");
    sqlx::query("insert into app_settings (key, value) values ('embed_state', '{\"model\": \"m\"}')").execute(&pool).await.unwrap();
    let empty = TagCentroids::current(&pool, &c, None).await.unwrap().unwrap();
    assert_eq!((empty.model.as_str(), empty.len()), ("m", 0));
    // once items gain embeddings, an empty cached build is rebuilt instead of carried over for 6 hours
    let v = boothdex_core::embed::HalfVector::from_f32_slice(&unit(1));
    for id in 1..=5i64 {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at, embedding) values ($1, 'A', 'c', 'u', false, 'ok', now(), $2::halfvec)")
            .bind(id)
            .bind(&v)
            .execute(&pool)
            .await
            .unwrap();
    }
    let built = TagCentroids::current(&pool, &c, Some(empty)).await.unwrap().unwrap();
    assert_eq!(built.len(), 1);
    let again = TagCentroids::current(&pool, &c, Some(built.clone())).await.unwrap().unwrap();
    assert!(Arc::ptr_eq(&built, &again));
    let old = Arc::new(TagCentroids { built: chrono::Utc::now() - REBUILD_AFTER, keys: vec!["a".into()], vectors: unit(1), model: "m".into() });
    assert!(!Arc::ptr_eq(&old, &TagCentroids::current(&pool, &c, Some(old.clone())).await.unwrap().unwrap()));
    sqlx::query("update app_settings set value = '{\"model\": \"n\"}' where key = 'embed_state'").execute(&pool).await.unwrap();
    assert_eq!(TagCentroids::current(&pool, &c, Some(built)).await.unwrap().unwrap().model, "n");
}
