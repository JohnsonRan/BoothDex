use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

use crate::error::Refusal;
use crate::{ApiError, AppState};

pub(super) const MAX_FAILURES: u32 = 10;
const LOCKOUT: Duration = Duration::from_secs(15 * 60);

/// Failed password checks per account (its lowercased username), or per typed name when no account matches: every
/// refusal calls `fail`, so known and unknown names do the same work. (count, window start)
#[derive(Default)]
pub struct LoginThrottle(Mutex<HashMap<String, (u32, Instant)>>);

impl LoginThrottle {
    pub(crate) fn blocked(&self, key: &str) -> bool {
        self.blocked_at(key, Instant::now())
    }
    fn blocked_at(&self, key: &str, now: Instant) -> bool {
        let map = self.0.lock().unwrap();
        map.get(key).is_some_and(|(n, t)| *n >= MAX_FAILURES && now.duration_since(*t) < LOCKOUT)
    }
    /// True when this failure locks the key: once per window, at exactly `MAX_FAILURES`.
    pub(crate) fn fail(&self, key: &str) -> bool {
        self.fail_at(key, Instant::now())
    }
    fn fail_at(&self, key: &str, now: Instant) -> bool {
        let mut map = self.0.lock().unwrap();
        // Keep every active window: evicting one would reset its login lockout.
        map.retain(|_, (_, start)| now.duration_since(*start) < LOCKOUT);
        let e = map.entry(key.to_owned()).or_insert((0, now));
        e.0 += 1;
        e.0 == MAX_FAILURES
    }
    pub(crate) fn clear(&self, key: &str) {
        self.0.lock().unwrap().remove(key);
    }
}

static ARGON2_SEMAPHORE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

async fn argon2<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, ApiError> {
    let permit = ARGON2_SEMAPHORE.acquire().await.map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f()
    })
    .await?)
}

pub async fn hash_password(password: String) -> Result<String, ApiError> {
    argon2(move || Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string()))
        .await?
        .map_err(|e| anyhow::anyhow!("hash: {e}").into())
}

pub(super) async fn verify_password(password: String, hash: String) -> Result<bool, ApiError> {
    argon2(move || Argon2::default().verify_password(password.as_bytes(), hash.as_str()).is_ok()).await
}

pub(super) fn valid_password(p: &str) -> bool {
    (10..=256).contains(&p.chars().count())
}

/// The signed-in account's own password, checked under the same limits as a sign-in: each check is an Argon2 hash
/// drawn from the IP's `auth` bucket, and failures count against the username, so a session cookie alone can't be
/// used to guess the password and take the account over.
pub(super) async fn check_own_password(s: &AppState, ip: IpAddr, user_id: i64, password: String, wrong: Refusal) -> Result<(), ApiError> {
    s.limits.auth.check(ip)?;
    let (username, hash): (String, String) =
        sqlx::query_as("select username, password_hash from users where id = $1").bind(user_id).fetch_one(&s.pool).await?;
    let key = username.to_lowercase();
    // its 429 is visible: the caller is signed in to this account
    if s.throttle.blocked(&key) {
        return Err(Refusal::TooManyAttempts.into());
    }
    if !verify_password(password, hash).await? {
        let locked_now = s.throttle.fail(&key);
        log_failure("password check failed", Some(&key), ip, locked_now);
        return Err(wrong.into());
    }
    s.throttle.clear(&key);
    Ok(())
}

/// One sign-in log line on stdout: `{event}: {who} from {ip}`. `account` is the lowercased username from the
/// database; no caller passes the typed name, because a member may type a password into the name field.
pub(super) fn log_attempt(event: &str, account: Option<&str>, ip: IpAddr) {
    println!("{event}: {} from {ip}", who(account));
}

/// A refused password check; `locked_now` comes from `LoginThrottle::fail` and adds a warning on stderr.
pub(super) fn log_failure(event: &str, account: Option<&str>, ip: IpAddr, locked_now: bool) {
    log_attempt(event, account, ip);
    if locked_now {
        eprintln!("warn: sign-in locked: {} after {MAX_FAILURES} failed password checks, the last from {ip}", who(account));
    }
}

fn who(account: Option<&str>) -> String {
    account.map_or_else(|| "unknown account".to_owned(), |a| format!("account {a}"))
}

/// Hash used to equalise login timing for unknown users.
pub fn dummy_hash() -> String {
    Argon2::default().hash_password(b"timing-equaliser-not-a-password").expect("argon2").to_string()
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use axum::Json;
    use sqlx::PgPool;

    use super::*;
    use crate::account::profile::{ChangePassword, DeleteAccount, change_password, delete_account};
    use crate::account::test_state;
    use crate::account::tests::{headers, signed_in};

    #[test]
    fn throttle_blocks_after_limit() {
        let t = LoginThrottle::default();
        for n in 1..=MAX_FAILURES {
            assert!(!t.blocked("x"));
            assert_eq!(t.fail("x"), n == MAX_FAILURES);
        }
        assert!(t.blocked("x"));
        assert!(!t.fail("x"));
        t.clear("x");
        assert!(!t.blocked("x"));
    }

    #[test]
    fn throttle_prunes_expired_names_without_resetting_active_windows() {
        let t = LoginThrottle::default();
        let expired = Instant::now();
        let start = expired + LOCKOUT / 2;
        let now = expired + LOCKOUT;
        {
            let mut map = t.0.lock().unwrap();
            map.insert("expired".into(), (MAX_FAILURES, expired));
            map.insert("locked".into(), (MAX_FAILURES, start));
            map.insert("active".into(), (MAX_FAILURES - 1, start));
        }
        t.fail_at("active", now);
        t.fail_at("new", now);
        let map = t.0.lock().unwrap();
        assert_eq!(map.len(), 3);
        assert!(!map.contains_key("expired"));
        assert_eq!(map["locked"], (MAX_FAILURES, start));
        assert_eq!(map["active"], (MAX_FAILURES, start));
        assert_eq!(map["new"].0, 1);
        drop(map);
        assert!(t.blocked_at("locked", now));
        assert!(t.blocked_at("active", now));
    }

    #[test]
    fn throttle_restarts_an_expired_names_window() {
        let t = LoginThrottle::default();
        let start = Instant::now();
        let now = start + LOCKOUT;
        t.0.lock().unwrap().insert("expired".into(), (MAX_FAILURES, start));
        assert!(t.blocked_at("expired", now - Duration::from_nanos(1)));
        assert!(!t.blocked_at("expired", now));
        t.fail_at("expired", now);
        assert_eq!(t.0.lock().unwrap()["expired"], (1, now));
        assert!(!t.blocked_at("expired", now));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn argon2_permit_stays_held_when_awaiting_future_is_dropped() {
        static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = TEST_MUTEX.lock().await;

        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ARGON2_SEMAPHORE.available_permits() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("baseline permits available");

        assert_eq!(ARGON2_SEMAPHORE.available_permits(), 4);

        let task = tokio::spawn(async move {
            argon2(move || {
                let _ = entered_tx.send(());
                let _ = release_rx.recv();
            })
            .await
        });

        entered_rx.await.expect("closure entered");
        assert_eq!(ARGON2_SEMAPHORE.available_permits(), 3);

        task.abort();
        let _ = task.await;

        assert_eq!(
            ARGON2_SEMAPHORE.available_permits(),
            3,
            "permit must not be released on future drop while closure is running"
        );

        release_tx.send(()).expect("release closure");

        let returned = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ARGON2_SEMAPHORE.available_permits() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(returned.is_ok(), "permit must return after closure finishes");
        assert_eq!(ARGON2_SEMAPHORE.available_permits(), 4);
    }

    #[sqlx::test(migrations = "../core/migrations")]
    #[ignore = "requires PostgreSQL with pgvector"]
    async fn password_checks_are_limited_like_sign_ins(pool: PgPool) {
        signed_in(&pool).await;
        let state = test_state(pool.clone());
        let change = |ip: [u8; 4]| {
            change_password(State(state.clone()), crate::limit::ClientIp(ip.into()), headers(), Json(ChangePassword {
                current: "not-the-password".into(),
                new: "0123456789".into(),
            }))
        };
        let status = |r: Result<Response, ApiError>| r.into_response().status();

        // an IP out of sign-in attempts can't check passwords either
        let busy = [203, 0, 113, 1];
        while state.limits.auth.check(busy.into()).is_ok() {}
        assert_eq!(status(change(busy).await), StatusCode::TOO_MANY_REQUESTS);
        let delete = delete_account(State(state.clone()), crate::limit::ClientIp(busy.into()), headers(), Json(DeleteAccount { password: "x".into() }));
        assert_eq!(status(delete.await), StatusCode::TOO_MANY_REQUESTS);

        // wrong guesses from many IPs lock the account's name, as failed sign-ins do
        for i in 0..MAX_FAILURES as u8 {
            assert_eq!(status(change([198, 51, 100, i]).await), StatusCode::FORBIDDEN);
        }
        assert_eq!(status(change([198, 51, 100, 200]).await), StatusCode::TOO_MANY_REQUESTS);
        assert!(state.throttle.blocked("a"));
    }
}
