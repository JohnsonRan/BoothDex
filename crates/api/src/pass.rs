//! The pass that gives a visitor today's rate limits back while the admin's switch for stricter limits is on
//! (limit.rs `Tiered`). A visitor earns it by passing a Turnstile check (`POST /api/verify`), and keeps it as a cookie
//! for 10 minutes. The cookie names the visitor's address range (an IPv4 /24, an IPv6 /64) and its expiry, signed with
//! HMAC-SHA256 by a key this process makes at start: a pass counts only with that signature, before its expiry and
//! from an address in its range, and a restart ends every pass, after which the visitor checks again.

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

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| s.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect()
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
        let passed = account::cookie(&parts.headers, COOKIE).is_some_and(|value| s.pass_key.valid(&value, ip, now()));
        Ok(if passed { Self::Base } else { Self::Strict(config) })
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
