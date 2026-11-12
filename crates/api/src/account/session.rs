use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{cookie_token, require_user, token_hash};
use crate::limit::is_internal;
use crate::{ApiError, AppState};

const COOKIE: &str = "session";
const KEEP_DAYS: i64 = 30;
const SHORT_HOURS: i64 = 12;

fn secure_flag() -> &'static str {
    if std::env::var("COOKIE_SECURE").as_deref() == Ok("0") { "" } else { "; Secure" }
}

/// Inserts the session ($1-$5) and stamps the account's last sign-in.
const INSERT_SESSION: &str = "with s as (insert into sessions (token_hash, user_id, expires_at, user_agent, ip) values ($1, $2, now() + $3, $4, $5) returning user_id)
     update users set last_sign_in_at = now() from s where users.id = s.user_id";

/// Creates a session row and returns the Set-Cookie value.
/// If `verified_hash` is Some, the session is only inserted if the user's current password hash matches it and the
/// account isn't disabled, preventing an in-flight login from creating a session after a concurrent reset or disable.
/// `ip` is the limiter's client IP; an internal one (web's container or a LAN address) is stored as NULL.
pub(super) async fn start_session(
    pool: &PgPool,
    user_id: i64,
    keep: bool,
    headers: &HeaderMap,
    ip: IpAddr,
    verified_hash: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).map_err(|e| anyhow::anyhow!("rng: {e}"))?;
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let ttl = if keep { chrono::Duration::days(KEEP_DAYS) } else { chrono::Duration::hours(SHORT_HOURS) };
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|s| s.chars().take(300).collect::<String>());
    let ip = Some(ip).filter(|ip| !is_internal(*ip)).map(|ip| ip.to_string());

    match verified_hash {
        Some(expected_hash) => {
            // Row lock on `users` serializes with admin `reset_password` and disable (both `UPDATE users`, then delete sessions):
            // if the admin commits first, `FOR UPDATE` sees the new hash or disabled_at and rejects;
            // if login locks first, the admin's UPDATE blocks until login commits, and then its DELETE removes this session.
            let mut tx = pool.begin().await?;
            let current_hash: Option<String> = sqlx::query_scalar(
                "select password_hash from users where id = $1 and disabled_at is null for update",
            )
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;

            if current_hash.as_deref() != Some(expected_hash) {
                return Ok(None);
            }

            sqlx::query(INSERT_SESSION)
                .bind(token_hash(&token))
                .bind(user_id)
                .bind(ttl)
                .bind(ua)
                .bind(ip)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;
        }
        None => {
            sqlx::query(INSERT_SESSION)
                .bind(token_hash(&token))
                .bind(user_id)
                .bind(ttl)
                .bind(ua)
                .bind(ip)
                .execute(pool)
                .await?;
        }
    }

    let max_age = if keep { format!("; Max-Age={}", KEEP_DAYS * 86_400) } else { String::new() };
    Ok(Some(format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax{}{max_age}", secure_flag())))
}

pub(super) fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax{}; Max-Age=0", secure_flag())
}

/// The account's live sessions: the one making the request first (`current`), then newest first.
pub async fn sessions_json(pool: &PgPool, user_id: i64, current: Option<Vec<u8>>) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'created_at', created_at, 'expires_at', expires_at,
                  'user_agent', user_agent, 'ip', ip, 'current', coalesce(token_hash = $2, false))
                  order by coalesce(token_hash = $2, false) desc, created_at desc, id desc), '[]')
         from sessions where user_id = $1 and expires_at > now()",
    )
    .bind(user_id)
    .bind(current)
    .fetch_one(pool)
    .await
}

pub(super) async fn logout(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Some(token) = cookie_token(&headers) {
        sqlx::query("delete from sessions where token_hash = $1").bind(token_hash(&token)).execute(&s.pool).await?;
    }
    Ok(([(header::SET_COOKIE, clear_cookie())], Json(json!({ "ok": true }))).into_response())
}

pub(super) async fn list_sessions(State(s): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    let current = cookie_token(&headers).map(|t| token_hash(&t));
    Ok(Json(json!({ "sessions": sessions_json(&s.pool, u.id, current).await? })).into_response())
}

pub(super) async fn remove_session(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let u = require_user(&s, &headers).await?;
    sqlx::query("delete from sessions where id = $1 and user_id = $2").bind(id).bind(u.id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::test_state;

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn people_see_and_sign_out_only_their_own_sessions(pool: PgPool) {
        let users: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash)
             values ('a', 'a@example.test', 'A', 'x'), ('b', 'b@example.test', 'B', 'x') returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let ids: Vec<i64> = sqlx::query_scalar(
            "insert into sessions (token_hash, user_id, expires_at) values ($1, $3, now() + interval '1 day'), ($2, $3, now() + interval '1 day'),
               ($4, $5, now() + interval '1 day') returning id",
        )
        .bind(token_hash("tok-a"))
        .bind(token_hash("tok-a2"))
        .bind(users[0])
        .bind(token_hash("tok-b"))
        .bind(users[1])
        .fetch_all(&pool)
        .await
        .unwrap();
        // this device signed in before the other one, and is still listed first
        sqlx::query("update sessions set created_at = now() - interval '1 hour' where id = $1").bind(ids[0]).execute(&pool).await.unwrap();
        let state = test_state(pool.clone());
        let mut a = HeaderMap::new();
        a.insert(header::COOKIE, "session=tok-a".parse().unwrap());

        let res = list_sessions(State(state.clone()), a.clone()).await.ok().unwrap();
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap();
        let current: Vec<bool> = body["sessions"].as_array().unwrap().iter().map(|s| s["current"].as_bool().unwrap()).collect();
        assert_eq!(current, [true, false]);

        // b's session is left alone; a's other one goes
        remove_session(State(state.clone()), a.clone(), Path(ids[2])).await.ok().unwrap();
        remove_session(State(state.clone()), a.clone(), Path(ids[1])).await.ok().unwrap();
        let left: Vec<i64> = sqlx::query_scalar("select id from sessions order by id").fetch_all(&pool).await.unwrap();
        assert_eq!(left, [ids[0], ids[2]]);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_session_stores_the_client_ip_not_a_forged_header(pool: PgPool) {
        use crate::account::{Login, Register, hash_password, login, register};
        use crate::limit::{ClientIp, client_ip};

        sqlx::query("insert into users (username, email, display_name, password_hash) values ('ann', 'ann@example.test', 'Ann', $1)")
            .bind(hash_password("0123456789".into()).await.ok().unwrap())
            .execute(&pool)
            .await
            .unwrap();
        let state = test_state(pool.clone());
        let request = |pairs: &[(&'static str, &str)], peer: [u8; 4]| {
            let mut h = HeaderMap::new();
            for (name, value) in pairs {
                h.insert(*name, value.parse().unwrap());
            }
            (ClientIp(client_ip(&h, Some(peer.into()))), h)
        };
        let sign_in = |(ip, h): (ClientIp, HeaderMap)| {
            login(State(state.clone()), ip, h, Json(Login { login: "ann".into(), password: "0123456789".into(), keep: false }))
        };
        let ok = |r: Result<Response, ApiError>| assert_eq!(r.into_response().status(), axum::http::StatusCode::OK);

        // behind Caddy or nginx: the proxy appended the visitor's address last; the header is forged
        ok(sign_in(request(&[("x-forwarded-for", "198.51.100.7, 203.0.113.42"), ("cf-connecting-ip", "8.8.8.8")], [172, 18, 0, 5])).await);
        let (ip, h) = request(&[("x-forwarded-for", "2001:DB8::1"), ("cf-connecting-ip", "8.8.8.8")], [10, 89, 0, 5]);
        let form = Register { display_name: "Bob".into(), username: "bob".into(), email: "bob@example.test".into(), password: "0123456789".into(), keep: false };
        ok(register(State(state.clone()), ip, h, Json(form)).await);
        // a LAN visitor: client_ip answers web's container address, which is not stored
        ok(sign_in(request(&[("x-forwarded-for", "192.168.1.20")], [172, 18, 0, 5])).await);

        let ips: Vec<Option<String>> = sqlx::query_scalar("select ip from sessions order by id").fetch_all(&pool).await.unwrap();
        assert_eq!(ips, [Some("203.0.113.42".into()), Some("2001:db8::1".into()), None]);
    }
}
