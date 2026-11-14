//! Through the router `main` serves: writes that the browser marks as cross-origin are refused on every route.

/// A write the browser marks as coming from another origin, a same-site subdomain included, is refused on account
/// and admin routes before it changes anything; the site's own calls, direct requests and other clients pass.
#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn writes_from_another_origin_are_refused(pool: sqlx::PgPool) {
    use serde_json::{Value, json};

    let boss: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash, is_admin) values ('boss', 'boss@example.test', 'Boss', 'x', true) returning id")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
        .bind(crate::account::token_hash("tok-boss"))
        .bind(boss)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at) select g, 't', 'c', 'https://example.test', false, 'ok', now() from generate_series(1, 3) g")
        .execute(&pool)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = crate::app(crate::account::test_state(pool.clone()));
    tokio::spawn(async move { axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await });
    let http = reqwest::Client::new();
    let send = |method: &str, path: &str, site: Option<&str>| {
        let mut request = http.request(method.parse().unwrap(), format!("{base}{path}")).header("cookie", "session=tok-boss");
        if let Some(site) = site {
            request = request.header("sec-fetch-site", site);
        }
        request.send()
    };
    let new_job = || sqlx::query_scalar::<_, i64>("insert into jobs (kind) values ('refresh') returning id").fetch_one(&pool);
    let job_status = |id: i64| sqlx::query_scalar::<_, String>("select status from jobs where id = $1").bind(id).fetch_one(&pool);
    let favourites = || sqlx::query_scalar::<_, i64>("select count(*) from favourites").fetch_one(&pool);

    let queued = new_job().await.unwrap();
    for site in ["cross-site", "same-site"] {
        let refused = send("PUT", "/api/account/favourites/1", Some(site)).await.unwrap();
        assert_eq!(refused.status(), 403, "{site}");
        let body: Value = refused.json().await.unwrap();
        assert_eq!(body, json!({ "ok": false, "code": "cross_site_request", "error": "Requests from another site can't change anything." }));
        assert_eq!(send("POST", &format!("/api/admin/jobs/{queued}/cancel"), Some(site)).await.unwrap().status(), 403, "{site}");
    }
    assert_eq!(favourites().await.unwrap(), 0);
    assert_eq!(job_status(queued).await.unwrap(), "queued");
    // refused before the rate limits: a page that keeps sending them does not use up the visitor's writes (burst 30)
    for _ in 0..40 {
        assert_eq!(send("DELETE", "/api/account/views", Some("cross-site")).await.unwrap().status(), 403);
    }

    for (item, site) in [(1, None), (2, Some("same-origin")), (3, Some("none"))] {
        assert_eq!(send("PUT", &format!("/api/account/favourites/{item}"), site).await.unwrap().status(), 200, "{site:?}");
        let job = if item == 1 { queued } else { new_job().await.unwrap() };
        assert_eq!(send("POST", &format!("/api/admin/jobs/{job}/cancel"), site).await.unwrap().status(), 200, "{site:?}");
        assert_eq!(job_status(job).await.unwrap(), "cancelled");
    }
    assert_eq!(favourites().await.unwrap(), 3);

    // reading stays open to other sites: the public API answers them with CORS
    let read = http.get(format!("{base}/api/health")).header("sec-fetch-site", "cross-site").header("origin", "https://elsewhere.example").send().await.unwrap();
    assert_eq!(read.status(), 200);
    assert_eq!(read.headers()["access-control-allow-origin"], "*");
}
