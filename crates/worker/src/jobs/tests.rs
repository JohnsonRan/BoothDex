use boothdex_core::embed::{DIMS, HalfVector};
use sqlx::PgPool;

use super::crawl::LISTING_IMAGES_UPSERT;
use super::embed::{EMBED_ROWS_SELECT, EMBEDDING_UPDATE, EmbedRow};
use super::link_relisted;

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn listing_images_preserve_complete_galleries(pool: PgPool) {
    // 1 a full gallery from the item page; 2 an older listing gallery; 3 and 4 not stored yet, and not in items
    sqlx::query("insert into booth_images (item_id, images, complete, fetched_at) values (1, '{f1,f2,f3,f4,f5}', true, '2000-01-01'), (2, '{old}', false, '2000-01-01')")
        .execute(&pool)
        .await
        .unwrap();
    let galleries: [(i64, &[&str]); 4] = [(1, &["l1"]), (2, &["n1", "n2"]), (3, &["c", "a", "b"]), (4, &[])];
    let ids: Vec<i64> = galleries.iter().map(|g| g.0).collect();
    let imgs: Vec<String> = galleries.iter().map(|g| serde_json::to_string(&g.1).unwrap()).collect();
    let expected = [
        (1, vec!["f1", "f2", "f3", "f4", "f5"], true, false),
        (2, vec!["n1", "n2"], false, true),
        (3, vec!["c", "a", "b"], false, true),
        (4, vec![], false, true),
    ];
    for _ in 0..2 {
        sqlx::query(LISTING_IMAGES_UPSERT).bind(&ids).bind(&imgs).execute(&pool).await.unwrap();
        let stored: Vec<(i64, Vec<String>, bool, bool)> =
            sqlx::query_as("select item_id, images, complete, fetched_at > '2000-01-01' from booth_images order by item_id").fetch_all(&pool).await.unwrap();
        assert_eq!(stored, expected.iter().map(|(id, images, complete, fresh)| (*id, images.iter().map(|s| s.to_string()).collect(), *complete, *fresh)).collect::<Vec<_>>());
    }
    let empty = sqlx::query(LISTING_IMAGES_UPSERT).bind(Vec::<i64>::new()).bind(Vec::<String>::new()).execute(&pool).await.unwrap().rows_affected();
    assert_eq!(empty, 0);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn binary_embedding_batch_skips_changed_items(pool: PgPool) {
    sqlx::query(
        "insert into items (id, title, bio, author_name, author_uri, is_adult, status, scraped_at, price_jpy, booth_tags, translations)
         select id, 'ab', 'c', 'creator', 'https://example.test', false, 'ok', now(), 500, '{t1,t2}',
                case when id = 8 then '{}' else '{\"en\": {\"title\": \"English\"}, \"ko\": {\"title\": \"Korean\"}}' end::jsonb
         from generate_series(1, 8) as id",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rows: Vec<EmbedRow> = sqlx::query_as(EMBED_ROWS_SELECT).bind(i64::MAX).bind(8i64).fetch_all(&pool).await.unwrap();
    let (ids, fingerprints): (Vec<_>, Vec<_>) = rows.into_iter().map(|r| (r.0, r.5)).unzip();
    // 1 unchanged; 2 to 5 one input each; 6 only fields outside the text; 7 the title/bio boundary moves; 8 a missing English title becomes ''
    for edit in [
        "update items set title = 'edited during embedding' where id = 2",
        "update items set bio = 'edited during embedding' where id = 3",
        "update items set translations = jsonb_set(translations, '{en,title}', '\"Edited\"') where id = 4",
        "update items set booth_tags = '{t1,t3}' where id = 5",
        "update items set price_jpy = 600, translations = jsonb_set(translations, '{ko,title}', '\"Edited\"') where id = 6",
        "update items set title = 'a', bio = 'bc' where id = 7",
        "update items set translations = '{\"en\": {\"title\": \"\"}}' where id = 8",
    ] {
        sqlx::query(edit).execute(&pool).await.unwrap();
    }
    let values = vec![0.125; DIMS];
    let vectors = vec![HalfVector::from_f32_slice(&values); ids.len()];
    let written = sqlx::query(EMBEDDING_UPDATE).bind(&ids).bind(&vectors).bind(&fingerprints).execute(&pool).await.unwrap().rows_affected();
    assert_eq!(written, 2);
    let stored: Vec<(i64, bool)> = sqlx::query_as("select id, embedding is not null from items order by id").fetch_all(&pool).await.unwrap();
    assert_eq!(stored, [(1, true), (2, false), (3, false), (4, false), (5, false), (6, true), (7, false), (8, false)]);
    let same: bool = sqlx::query_scalar("select bool_and(halfvec_send(embedding) = halfvec_send($1::halfvec)) from items where embedding is not null")
        .bind(&vectors[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(same);
    let empty = sqlx::query(EMBEDDING_UPDATE).bind(Vec::<i64>::new()).bind(Vec::<HalfVector>::new()).bind(Vec::<String>::new()).execute(&pool).await.unwrap().rows_affected();
    assert_eq!(empty, 0);
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn delisted_items_point_at_their_re_upload(pool: PgPool) {
    // 10 delisted; 9 an older copy; 11 and 12 re-uploads (12 newest); 13 another shop; 14 another title
    for (id, shop, title, status) in [
        (9, "a", "Foo Bar", "ok"),
        (10, "a", "Foo Bar", "delisted"),
        (11, "a", "foo  bar", "ok"),
        (12, "a", "FOO BAR", "ok"),
        (13, "b", "Foo Bar", "ok"),
        (14, "a", "Foo Baz", "ok"),
    ] {
        sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at) values ($1, $3, 's', $2, false, $4, now())")
            .bind(id as i64)
            .bind(shop)
            .bind(title)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
    }
    let link = || async { sqlx::query_scalar::<_, Option<i64>>("select relisted_as from items where id = 10").fetch_one(&pool).await.unwrap() };
    assert_eq!(link_relisted(&pool).await.unwrap(), 1);
    assert_eq!(link().await, Some(12));
    assert_eq!(link_relisted(&pool).await.unwrap(), 0);
    sqlx::query("update items set status = 'delisted' where id = 12").execute(&pool).await.unwrap();
    link_relisted(&pool).await.unwrap();
    assert_eq!(link().await, Some(11));
    sqlx::query("update items set status = 'ok' where id = 10").execute(&pool).await.unwrap();
    link_relisted(&pool).await.unwrap();
    assert_eq!(link().await, None);
    let stray: i64 = sqlx::query_scalar("select count(*) from items where relisted_as is not null").fetch_one(&pool).await.unwrap();
    assert_eq!(stray, 0);
}
