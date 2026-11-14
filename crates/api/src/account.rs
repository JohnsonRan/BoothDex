//! Password accounts, cookie sessions (each listed and signed out on its own), preferences, favourites and saved
//! searches. An admin can disable an account (admin/users.rs) or close registration (app_settings 'registration').
//! Not yet: Discord/X OAuth (needs app credentials) and password-reset email (needs a mail service).
//!
//! CSRF: the session cookie is SameSite=Lax and CORS never allows credentials, so a request from another site
//! carries no cookie. A page on another subdomain of the same site does send it, so every API request other than
//! GET, HEAD or OPTIONS that the browser marks as not same-origin (`Sec-Fetch-Site`) is refused before any route runs
//! (`refuse_cross_site_writes`); a request without that header, from another client or an old browser, relies on
//! SameSite alone. Body-carrying mutations additionally require JSON.

mod favourites;
mod password;
mod profile;
mod searches;
mod session;
mod sign_in;
mod views;

use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Router;
#[cfg(test)]
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

pub(crate) use favourites::{FavouriteCache, favourite_ids};
use favourites::{add_favourite, list_favourites, remove_favourite};
pub(crate) use password::{LoginThrottle, dummy_hash, hash_password};
use profile::{change_password, delete_account, get_preferences, me, put_preferences, update_profile};
use searches::{add_search, list_searches, remove_search, rename_search};
pub(crate) use session::sessions_json;
use session::{list_sessions, logout, remove_session};
#[cfg(test)]
pub(crate) use sign_in::{Login, Register};
pub(crate) use sign_in::{login, register, registration_open};
use sign_in::registration;
use views::{add_views, clear_views, list_views};

use crate::error::Refusal;
use crate::limit::Check;
use crate::{ApiError, AppState, limit};

pub struct User {
    pub id: i64,
}

pub(crate) fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| kv.trim().strip_prefix("session=").map(str::to_owned))
}

pub async fn current_user(pool: &PgPool, headers: &HeaderMap) -> Result<Option<User>, ApiError> {
    let Some(token) = cookie_token(headers) else { return Ok(None) };
    let id: Option<i64> = sqlx::query_scalar("select user_id from sessions where token_hash = $1 and expires_at > now()")
        .bind(token_hash(&token))
        .fetch_optional(pool)
        .await?;
    Ok(id.map(|id| User { id }))
}

async fn require_user(s: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    Ok(current_user(&s.pool, headers).await?.ok_or(Refusal::SignInRequired)?)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // public: signing up, in and out, and whether sign-up is open; every later handler reads the session itself
        .route("/api/account/register", post(register))
        .route("/api/account/login", post(login))
        .route("/api/account/logout", post(logout))
        .route("/api/account/registration", get(registration))
        .route("/api/account/sessions", get(list_sessions))
        .route("/api/account/sessions/{id}", axum::routing::delete(remove_session))
        .route("/api/account", get(me).delete(delete_account))
        .route("/api/account/profile", axum::routing::patch(update_profile))
        .route("/api/account/password", post(change_password))
        .route("/api/account/preferences", get(get_preferences).put(put_preferences))
        .route("/api/account/favourites", get(list_favourites))
        .route("/api/account/favourites/{id}", put(add_favourite).delete(remove_favourite))
        .route("/api/account/views", get(list_views).post(add_views).delete(clear_views))
        .route("/api/account/searches", get(list_searches).post(add_search))
        .route("/api/account/searches/{id}", axum::routing::patch(rename_search).delete(remove_search))
}

/// Account changes are database writes; reading the account is not limited.
pub(crate) async fn account_write_rate_limit(
    State(s): State<Arc<AppState>>,
    limit::ClientIp(ip): limit::ClientIp,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if req.method() != axum::http::Method::GET && let Err(r) = s.limits.account.check(ip) {
        return r.into_response();
    }
    next.run(req).await
}

/// Refuses a write the browser sent from another origin, same-site subdomains included. `none` is a request the
/// visitor made directly (typed in the address bar); the site's own calls arrive as `same-origin` through its rewrite.
pub(crate) async fn refuse_cross_site_writes(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    use axum::http::Method;
    let write = ![Method::GET, Method::HEAD, Method::OPTIONS].contains(req.method());
    let cross = req.headers().get("sec-fetch-site").is_some_and(|v| v != "same-origin" && v != "none");
    if write && cross {
        return ApiError::from(Refusal::CrossSiteRequest).into_response();
    }
    next.run(req).await
}

/// An API over `pool` with an empty catalogue, for handler tests.
#[cfg(test)]
pub(crate) fn test_state(pool: PgPool) -> Arc<AppState> {
    crate::install_tls();
    Arc::new(
        AppState::new(
            pool,
            crate::Snapshot {
                catalog: crate::catalog::Catalog::build(Vec::new(), 0, None).unwrap(),
                category_phrases: Default::default(),
                avatars: crate::Lookup::new(Vec::new()),
                authors: crate::Lookup::new(Vec::new()),
                tags: crate::Lookup::new(Vec::new()),
                booth_tags: crate::Lookup::new(Vec::new()),
                categories: json!({ "categories": [] }),
                version: 1,
                loaded_at: chrono::Utc::now(),
                build_secs: 0.0,
            },
            Default::default(),
            "http://127.0.0.1:3000".into(),
            None,
            dummy_hash(),
            Some(Duration::from_secs(2)),
        )
        .unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use axum::Json;
    use serde_json::Value;

    use super::*;
    use super::password::MAX_FAILURES;
    use super::profile::DeleteAccount;
    use super::searches::{MAX_SAVED_SEARCHES, NewSearch};

    #[test]
    fn cookie_parsing() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "a=1; session=abc123; b=2".parse().unwrap());
        assert_eq!(cookie_token(&h).as_deref(), Some("abc123"));
    }

    /// Account 'a', signed in with the cookie `headers()` sends.
    pub(super) async fn signed_in(pool: &PgPool) -> i64 {
        let user = sqlx::query_scalar("insert into users (username, email, display_name, password_hash) values ('a', 'a@example.test', 'A', 'x') returning id")
            .fetch_one(pool)
            .await
            .unwrap();
        sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, now() + interval '1 day')")
            .bind(token_hash("tok-a"))
            .bind(user)
            .execute(pool)
            .await
            .unwrap();
        user
    }

    pub(super) fn headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "session=tok-a".parse().unwrap());
        h
    }

    /// Listed items 1 to n.
    pub(super) async fn items(pool: &PgPool, n: i64) {
        sqlx::query(
            "insert into items (id, title, author_name, author_uri, is_adult, status, scraped_at)
             select g, 't', 'c', 'https://example.test', false, 'ok', now() from generate_series(1, $1) g",
        )
        .bind(n)
        .execute(pool)
        .await
        .unwrap();
    }

    /// The `ids` of a list answer.
    pub(super) async fn ids(r: Response) -> Vec<i64> {
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap()).unwrap();
        serde_json::from_value(body["ids"].clone()).unwrap()
    }

    /// An answer as a client reads it: its status and its JSON body.
    pub(super) async fn answer(r: Result<Response, ApiError>) -> (StatusCode, Value) {
        let r = r.into_response();
        let status = r.status();
        (status, serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap())
    }

    /// The site and other clients read a refusal's status and its `error` text (web lib/messages.ts matches the text).
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn refusals_keep_their_status_and_message(pool: PgPool) {
        signed_in(&pool).await;
        let state = test_state(pool.clone());
        let ip = crate::limit::ClientIp([203, 0, 113, 20].into());
        let refused = |status: StatusCode, code: &str, message: &str| (status, json!({ "ok": false, "code": code, "error": message }));
        let sign_in = |name: &str| login(State(state.clone()), ip, HeaderMap::new(), Json(Login { login: name.into(), password: "0123456789".into(), keep: false }));

        assert_eq!(answer(sign_in("nobody").await).await, refused(StatusCode::UNAUTHORIZED, "bad_credentials", "Incorrect username or password."));
        for _ in 0..MAX_FAILURES {
            state.throttle.fail("locked");
        }
        assert_eq!(answer(sign_in("locked").await).await, refused(StatusCode::UNAUTHORIZED, "bad_credentials", "Incorrect username or password."));

        // a sign-up form, from an address of its own: one address has five attempts
        let sign_up = |display_name: &str, username: &str, email: &str, password: &str| {
            let form = Register { display_name: display_name.into(), username: username.into(), email: email.into(), password: password.into(), keep: false };
            register(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 21].into()), HeaderMap::new(), Json(form))
        };
        let bad = |code: &str, message: &str| refused(StatusCode::BAD_REQUEST, code, message);
        assert_eq!(answer(sign_up(" ", "ann", "ann@example.test", "0123456789").await).await, bad("display_name_required", "Display name is required."));
        assert_eq!(answer(sign_up("Ann", "an", "ann@example.test", "0123456789").await).await, bad("bad_username", "Username must be 3-24 letters, digits or underscores."));
        assert_eq!(answer(sign_up("Ann", "ann", "ann", "0123456789").await).await, bad("bad_email", "Invalid email."));
        assert_eq!(answer(sign_up("Ann", "ann", "ann@example.test", "short").await).await, bad("password_too_short", "Your password must be at least 10 characters."));
        // account 'a' has this email
        assert_eq!(
            answer(sign_up("Ann", "ann", "a@example.test", "0123456789").await).await,
            refused(StatusCode::CONFLICT, "name_taken", "Username or email is already taken.")
        );

        let delete = delete_account(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 22].into()), headers(), Json(DeleteAccount { password: "x".into() }));
        assert_eq!(answer(delete.await).await, refused(StatusCode::FORBIDDEN, "wrong_password", "Password is incorrect."));

        sqlx::query("insert into saved_searches (user_id, name, query) select u.id, 'n', 'q=' || g from users u, generate_series(1, $1) g")
            .bind(MAX_SAVED_SEARCHES)
            .execute(&pool)
            .await
            .unwrap();
        let one_more = add_search(State(state.clone()), headers(), Json(NewSearch { name: "n".into(), query: "q=x".into() }));
        assert_eq!(answer(one_more.await).await, refused(StatusCode::CONFLICT, "saved_search_limit", "Saved search limit reached."));

        assert_eq!(answer(list_favourites(State(state.clone()), HeaderMap::new()).await).await, refused(StatusCode::UNAUTHORIZED, "sign_in_required", "Sign in required"));
        assert_eq!(answer(add_favourite(State(state.clone()), headers(), Path(999)).await).await, refused(StatusCode::NOT_FOUND, "unknown_item", "Unknown item."));
        assert_eq!(
            answer(put_preferences(State(state.clone()), headers(), Json(json!({ "evil": "x" }))).await).await,
            refused(StatusCode::BAD_REQUEST, "bad_preferences", "unknown preference")
        );
    }

    /// PostgreSQL text cannot hold a NUL: each free-text field refuses one before the statement would fail with a 500.
    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn a_nul_in_free_text_is_refused_not_failed(pool: PgPool) {
        signed_in(&pool).await;
        let state = test_state(pool.clone());
        let status = |r: Result<Response, ApiError>| r.into_response().status();
        let sign_in = login(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 30].into()), HeaderMap::new(), Json(Login {
            login: "a\0".into(),
            password: "0123456789".into(),
            keep: false,
        }));
        assert_eq!(status(sign_in.await), StatusCode::UNAUTHORIZED);
        let sign_up = register(State(state.clone()), crate::limit::ClientIp([203, 0, 113, 31].into()), HeaderMap::new(), Json(Register {
            display_name: "Ann\0".into(),
            username: "ann".into(),
            email: "ann@example.test".into(),
            password: "0123456789".into(),
            keep: false,
        }));
        assert_eq!(status(sign_up.await), StatusCode::BAD_REQUEST);
        let profile = serde_json::from_value(json!({ "display_name": "A\0" })).unwrap();
        assert_eq!(status(update_profile(State(state.clone()), headers(), Json(profile)).await), StatusCode::BAD_REQUEST);
        let add = |name: &str, query: &str| add_search(State(state.clone()), headers(), Json(NewSearch { name: name.into(), query: query.into() }));
        assert_eq!(status(add("n\0", "q=x").await), StatusCode::BAD_REQUEST);
        assert_eq!(status(add("n", "q=\0").await), StatusCode::BAD_REQUEST);
        assert_eq!(sqlx::query_scalar::<_, i64>("select count(*) from users").fetch_one(&pool).await.unwrap(), 1);
    }
}
