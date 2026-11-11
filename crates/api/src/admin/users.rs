//! User management: list and search accounts, one account's details and sessions, disable or enable an account,
//! sign out one of its sessions. Admins come from ADMIN_USERNAMES, so they can't be disabled here.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{err, require_admin};
use crate::{ApiError, AppState, account};

const PAGE: i64 = 50;

/// One account as the console shows it in a row (`u` is `users`); a macro, so the queries stay static strings.
macro_rules! user_json {
    () => {
        "jsonb_build_object('id', u.id, 'username', u.username, 'display_name', u.display_name, 'email', u.email,
         'is_admin', u.is_admin, 'disabled_at', u.disabled_at, 'created_at', u.created_at, 'last_sign_in_at', u.last_sign_in_at)"
    };
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: i64,
}

/// Admins first, then by sign-up, newest first; `PAGE` at a time. `q` matches part of the username, email or display
/// name, ignoring case.
pub(super) async fn list(State(s): State<Arc<AppState>>, headers: HeaderMap, Query(p): Query<ListQuery>) -> Result<Response, ApiError> {
    admin!(s, headers);
    let q: String = p.q.trim().to_lowercase().chars().take(100).collect();
    let v: Value = sqlx::query_scalar(concat!(
        "with m as (select * from users u
                    where $1 = '' or strpos(lower(u.username), $1) > 0 or strpos(lower(u.email), $1) > 0 or strpos(lower(u.display_name), $1) > 0)
         select jsonb_build_object('total', (select count(*) from m),
           'users', coalesce((select jsonb_agg(",
        user_json!(),
        " order by u.is_admin desc, u.created_at desc, u.id desc)
                              from (select * from m order by is_admin desc, created_at desc, id desc limit $2 offset $3) u), '[]'))"
    ))
    .bind(&q)
    .bind(PAGE)
    .bind(p.offset.max(0))
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(v).into_response())
}

/// One account with what it keeps and its live sessions.
pub(super) async fn get(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>) -> Result<Response, ApiError> {
    admin!(s, headers);
    let user: Option<Value> = sqlx::query_scalar(concat!(
        "select ",
        user_json!(),
        " || jsonb_build_object(
           'favourites', (select count(*) from favourites where user_id = u.id),
           'saved_searches', (select count(*) from saved_searches where user_id = u.id),
           'views', (select count(*) from item_views where user_id = u.id))
         from users u where u.id = $1"
    ))
    .bind(id)
    .fetch_optional(&s.pool)
    .await?;
    let Some(mut user) = user else { return Ok(err(StatusCode::NOT_FOUND, "No such user")) };
    user["sessions"] = account::sessions_json(&s.pool, id, None).await?;
    Ok(Json(user).into_response())
}

#[derive(Deserialize)]
pub(super) struct Patch {
    disabled: bool,
}

/// Disabling signs the account out everywhere; enabling lets it sign in again with its old password.
pub(super) async fn patch(State(s): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<i64>, Json(b): Json<Patch>) -> Result<Response, ApiError> {
    let who = admin!(s, headers);
    let mut tx = s.pool.begin().await?;
    // the row lock orders this against a sign-in in flight (account.rs start_session)
    let row: Option<(String, bool)> = sqlx::query_as("select username, is_admin from users where id = $1 for update").bind(id).fetch_optional(&mut *tx).await?;
    let Some((username, is_admin)) = row else { return Ok(err(StatusCode::NOT_FOUND, "No such user")) };
    if b.disabled && is_admin {
        return Ok(err(StatusCode::CONFLICT, "Admins can't be disabled."));
    }
    sqlx::query("update users set disabled_at = case when $2 then coalesce(disabled_at, now()) end where id = $1").bind(id).bind(b.disabled).execute(&mut *tx).await?;
    let revoked = if b.disabled { sqlx::query("delete from sessions where user_id = $1").bind(id).execute(&mut *tx).await?.rows_affected() } else { 0 };
    tx.commit().await?;
    println!("admin {who} {} {username}", if b.disabled { "disabled" } else { "enabled" });
    Ok(Json(json!({ "ok": true, "sessions_revoked": revoked })).into_response())
}

pub(super) async fn remove_session(State(s): State<Arc<AppState>>, headers: HeaderMap, Path((id, session)): Path<(i64, i64)>) -> Result<Response, ApiError> {
    admin!(s, headers);
    sqlx::query("delete from sessions where id = $1 and user_id = $2").bind(session).bind(id).execute(&s.pool).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[cfg(test)]
mod tests {
    use axum::http::header;

    use super::*;

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn admins_disable_accounts_and_sign_them_out(pool: sqlx::PgPool) {
        let ids: Vec<i64> = sqlx::query_scalar(
            "insert into users (username, email, display_name, password_hash, is_admin)
             values ('boss', 'boss@example.test', 'Boss', 'x', true), ('ann', 'ann@example.test', 'Ann', 'x', false) returning id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $3, now() + interval '1 day'), ($2, $4, now() + interval '1 day')")
            .bind(account::token_hash("tok-boss"))
            .bind(account::token_hash("tok-ann"))
            .bind(ids[0])
            .bind(ids[1])
            .execute(&pool)
            .await
            .unwrap();
        let state = account::test_state(pool.clone());
        let mut boss = HeaderMap::new();
        boss.insert(header::COOKIE, "session=tok-boss".parse().unwrap());
        let status = |r: Result<Response, ApiError>| r.ok().unwrap().status();
        let disable = |id: i64, disabled: bool| patch(State(state.clone()), boss.clone(), Path(id), Json(Patch { disabled }));

        assert_eq!(status(disable(ids[0], true).await), StatusCode::CONFLICT);
        assert_eq!(status(disable(0, true).await), StatusCode::NOT_FOUND);
        assert_eq!(status(disable(ids[1], true).await), StatusCode::OK);
        let ann: (bool, i64) = sqlx::query_as("select disabled_at is not null, (select count(*) from sessions where user_id = $1) from users where id = $1")
            .bind(ids[1])
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ann, (true, 0));
        // a non-admin can't call it
        let mut plain = HeaderMap::new();
        plain.insert(header::COOKIE, "session=nobody".parse().unwrap());
        assert_eq!(status(patch(State(state.clone()), plain, Path(ids[1]), Json(Patch { disabled: false })).await), StatusCode::UNAUTHORIZED);

        // search matches part of a name, ignoring case
        let found = list(State(state.clone()), boss.clone(), Query(ListQuery { q: " AN ".into(), offset: 0 })).await.ok().unwrap();
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(found.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert_eq!(body["total"], 1);
        assert_eq!(body["users"][0]["username"], "ann");
        assert!(body["users"][0]["disabled_at"].is_string());

        assert_eq!(status(disable(ids[1], false).await), StatusCode::OK);
        let disabled: bool = sqlx::query_scalar("select disabled_at is not null from users where id = $1").bind(ids[1]).fetch_one(&pool).await.unwrap();
        assert!(!disabled);

        // admins first, then the newest sign-ups: boss and ann signed up together, cy a day before
        sqlx::query(
            "insert into users (username, email, display_name, password_hash, created_at)
             values ('cy', 'cy@example.test', 'Cy', 'x', now() - interval '1 day')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let all = list(State(state.clone()), boss.clone(), Query(ListQuery { q: String::new(), offset: 0 })).await.ok().unwrap();
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(all.into_body(), 1 << 20).await.unwrap()).unwrap();
        let names: Vec<&str> = body["users"].as_array().unwrap().iter().map(|u| u["username"].as_str().unwrap()).collect();
        assert_eq!(names, ["boss", "ann", "cy"]);
    }
}
