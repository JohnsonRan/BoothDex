//! Gate for crates/AGENTS.md#authz: every admin route answers 401 to a signed-out visitor and 403 to a member.
//! The routes are read from `routes()` in admin.rs, so a route added there later is checked as well.

use crate::account;

/// One body every admin payload accepts (serde skips the fields a payload lacks), so extraction succeeds and the
/// handler's own check answers. A new payload with another required field answers 422 here until it is added.
const BODY: &str = r#"{"kind": "x", "open": false, "username": "x", "disabled": false}"#;

/// Method and path of every route in `routes()`, with each path parameter set to 1. Panics on a route it cannot
/// read, so a route written in another shape fails the test instead of going unchecked.
fn admin_routes() -> Vec<(String, String)> {
    let source = include_str!("../admin.rs");
    let lines = source.lines().skip_while(|l| !l.starts_with("pub fn routes()")).take_while(|l| *l != "}");
    let mut found = Vec::new();
    for line in lines.map(str::trim) {
        if ![".route(", ".nest(", ".merge("].iter().any(|call| line.contains(call)) {
            continue;
        }
        let Some(rest) = line.strip_prefix(".route(\"") else { unreadable(line) };
        let (path, handlers) = rest.split_once('"').unwrap();
        let before = found.len();
        let mut path = path.to_owned();
        while let Some(open) = path.find('{') {
            let close = open + path[open..].find('}').unwrap();
            path.replace_range(open..=close, "1");
        }
        // in `get(a).patch(b)` and `axum::routing::delete(c)`, the word before each "(" is a method or a handler
        let words: Vec<&str> = handlers.split('(').collect();
        for word in &words[..words.len() - 1] {
            let word = word.rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_').next().unwrap();
            if ["get", "post", "put", "patch", "delete"].contains(&word) {
                found.push((word.to_uppercase(), path.clone()));
            }
        }
        if found.len() == before {
            unreadable(line);
        }
    }
    found
}

fn unreadable(line: &str) -> ! {
    panic!("cannot read this route; write it on one line as .route(\"path\", get(handler)): {line}");
}

#[sqlx::test(migrations = "../core/migrations")]
#[ignore = "requires PostgreSQL with pgvector"]
async fn admin_routes_refuse_visitors_and_members(pool: sqlx::PgPool) {
    let member: i64 = sqlx::query_scalar("insert into users (username, email, display_name, password_hash) values ('ann', 'ann@example.test', 'Ann', 'x') returning id")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
        .bind(account::token_hash("tok-ann"))
        .bind(member)
        .execute(&pool)
        .await
        .unwrap();
    let app = super::routes().with_state(account::test_state(pool));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    let http = reqwest::Client::new();

    let routes = admin_routes();
    assert!(!routes.is_empty(), "no route read from admin.rs");
    for (method, path) in routes {
        for (cookie, want) in [(None, 401), (Some("session=tok-ann"), 403)] {
            let mut request = http
                .request(method.parse().unwrap(), format!("{base}{path}"))
                .header("content-type", "application/json")
                .body(BODY);
            if let Some(cookie) = cookie {
                request = request.header("cookie", cookie);
            }
            assert_eq!(request.send().await.unwrap().status().as_u16(), want, "{method} {path}, cookie {cookie:?}");
        }
    }
}
