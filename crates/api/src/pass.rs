//! The pass that gives a visitor today's rate limits back while the admin's switch for stricter limits is on
//! (limit.rs `Tiered`). A visitor earns it by passing a Turnstile check (`POST /api/verify`), and keeps it as a cookie
//! for 10 minutes. The cookie names the visitor's address range (an IPv4 /24, an IPv6 /64) and its expiry, signed with
//! HMAC-SHA256 by a key this process makes at start: a pass counts only with that signature, before its expiry and
//! from an address in its range, and a restart ends every pass, after which the visitor checks again. While sign-in
//! and registration are checked too (turnstile.rs `auth_keys`), a session whose sign-in or registration passed that
//! check (`sessions.turnstile_passed`) gets today's limits without a pass; one let through unchecked does not.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::header;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use hmac::{KeyInit, Mac};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;

use crate::error::Refusal;
use crate::limit::{Check, ClientIp, Tier};
use crate::turnstile::{self, Verdict};
use crate::{ApiError, AppState, account};

const COOKIE: &str = "pass";
/// How long a pass lasts, in seconds.
const TTL: u64 = 600;
/// The action the site's widget names; a token minted for another form does not earn a pass.
pub(crate) const ACTION: &str = "pass";

type Hmac = hmac::Hmac<sha2::Sha256>;

pub(crate) struct PassKey([u8; 32]);

impl PassKey {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).map_err(|e| anyhow::anyhow!("rng: {e}"))?;
        Ok(Self(key))
    }

    fn mac(&self, range: &str, expiry: u64) -> Hmac {
        let mut mac = Hmac::new_from_slice(&self.0).expect("HMAC takes a key of any length");
        mac.update(format!("{range} {expiry}").as_bytes());
        mac
    }

    /// The cookie's value: `<range>.<expiry>.<signature in hex>`, the expiry in Unix seconds.
    fn issue(&self, ip: IpAddr, expiry: u64) -> String {
        let range = range(ip);
        let signature: String = self.mac(&range, expiry).finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect();
        format!("{range}.{expiry}.{signature}")
    }

    /// Whether `value` is a pass this process issued, unexpired at `now`, for the range `ip` is in.
    fn valid(&self, value: &str, ip: IpAddr, now: u64) -> bool {
        // the range holds dots of its own, the expiry and the signature none
        let mut parts = value.rsplitn(3, '.');
        let (Some(signature), Some(expiry), Some(range_part)) = (parts.next(), parts.next(), parts.next()) else { return false };
        let (Ok(expiry), Some(signature)) = (expiry.parse::<u64>(), unhex(signature)) else { return false };
        expiry > now && range_part == range(ip) && self.mac(range_part, expiry).verify_slice(&signature).is_ok()
    }
}

/// The range a pass covers: the /24 of an IPv4 address, the /64 of an IPv6 one, as the rate limits key IPv6.
fn range(ip: IpAddr) -> String {
    match ip.to_canonical() {
        IpAddr::V4(v4) => format!("{}/24", Ipv4Addr::from(u32::from(v4) & !0xff)),
        IpAddr::V6(v6) => format!("{}/64", Ipv6Addr::from(u128::from(v6) & !u128::from(u64::MAX))),
    }
}

/// The signature as `issue` writes it: exactly 64 lower-case hex digits, so each pass has one spelling.
fn unhex(s: &str) -> Option<Vec<u8>> {
    let nibble = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    if s.len() != 64 {
        return None;
    }
    s.as_bytes().chunks(2).map(|pair| Some(nibble(pair[0])? << 4 | nibble(pair[1])?)).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl FromRequestParts<Arc<AppState>> for Tier {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, s: &Arc<AppState>) -> Result<Self, ApiError> {
        let config = s.turnstile_config().await?;
        if config.strict_keys().is_none() {
            return Ok(Self::Base);
        }
        let Ok(ClientIp(ip)) = ClientIp::from_request_parts(parts, s).await;
        if account::cookie(&parts.headers, COOKIE).is_some_and(|value| s.pass_key.valid(&value, ip, now())) {
            return Ok(Self::Base);
        }
        // A session counts only while sign-in is checked (owner, 2026-11-17: "1. B"), and only if its own sign-in
        // passed the check; it is read only when the strict bucket refuses (limit.rs `Tiered::check`)
        let session = config.auth_keys().and(account::cookie_token(&parts.headers)).map(|token| Session { pool: s.pool.clone(), token });
        Ok(Self::Strict { config, session })
    }
}

/// The session cookie of a request to a tiered bucket while sign-in is Turnstile-checked.
pub struct Session {
    pool: PgPool,
    token: String,
}

impl Session {
    /// Whether the cookie names a live session whose sign-in or registration passed a Turnstile check. A database
    /// error counts as no such session, so the visitor keeps the stricter limits rather than getting a 500.
    pub(crate) async fn checked(&self) -> bool {
        let query = "select exists (select 1 from sessions where token_hash = $1 and expires_at > now() and turnstile_passed)";
        match sqlx::query_scalar(query).bind(account::token_hash(&self.token)).fetch_one(&self.pool).await {
            Ok(checked) => checked,
            Err(e) => {
                eprintln!("error: rate limit: reading the session failed, counted as none: {e}");
                false
            }
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct Verify {
    token: String,
}

/// `POST /api/verify`: a token from the site's widget (action `pass`) earns a pass. A token Cloudflare refuses is
/// answered 403 `verification_failed`; when siteverify cannot judge it, 503 `verification_unavailable` and no pass:
/// the visitor keeps the stricter limits, which still let them browse. Closed (503) while the switch is off.
pub(crate) async fn verify(State(s): State<Arc<AppState>>, ClientIp(ip): ClientIp, Json(b): Json<Verify>) -> Result<Response, ApiError> {
    let config = s.turnstile_config().await?;
    let Some((_, secret)) = config.strict_keys() else { return Err(Refusal::VerificationUnavailable.into()) };
    s.limits.verify.check(ip)?;
    match turnstile::verdict(&s, secret, &b.token, ip, ACTION).await {
        Verdict::Passed => {
            // Path=/: the browser sends the pass with the page and its navigations too, and the server's reads for them
            // pass it on (web lib/server-data.ts, `visitorHeaders`); under Path=/api those reads would stay strict
            let cookie = format!(
                "{COOKIE}={}; Path=/; HttpOnly; SameSite=Lax{}; Max-Age={TTL}",
                s.pass_key.issue(ip, now() + TTL),
                account::secure_flag()
            );
            Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
        }
        Verdict::Refused => Err(Refusal::VerificationFailed.into()),
        Verdict::Unjudged => Err(Refusal::VerificationUnavailable.into()),
    }
}

#[cfg(test)]
#[path = "pass_tests.rs"]
mod tests;
