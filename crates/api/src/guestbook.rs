//! The guestbook: anyone may post a message under a nickname after a Turnstile check. A message is final once posted
//! (the site asks the author to confirm first); every visitor only reads. An admin hides or deletes messages
//! (admin/guestbook.rs). The client IP of a post is logged, never stored.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::Refusal;
use crate::limit::{Check, ClientIp};
use crate::{ApiError, AppState, turnstile};

pub(crate) const NICKNAME_MAX: usize = 24;
pub(crate) const BODY_MAX: usize = 500;
const PAGE: i64 = 50;
/// The action the site's widget names; a token for another form does not pass here.
pub(crate) const ACTION: &str = "guestbook";

#[derive(Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct GuestbookEntry {
    pub(crate) id: i64,
    pub(crate) nickname: String,
    pub(crate) body: String,
    #[cfg_attr(test, ts(type = "string"))]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct GuestbookPage {
    /// newest first
    pub(crate) entries: Vec<GuestbookEntry>,
    /// older messages follow; ask again with `before` set to the last id
    pub(crate) more: bool,
    /// the widget's site key; null while the guestbook is closed (either Turnstile key unset)
    pub(crate) site_key: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    // public: anyone reads and posts
    Router::new().route("/api/guestbook", get(list).post(post))
}

/// Whether `t` holds a payment card number: 13 to 19 digits that pass the Luhn check, inside a run of digit groups
/// joined by single spaces, hyphens or dots. The number may be any stretch of whole groups in the run, so a CVV or
/// expiry written after it ("4111 1111 1111 1111 123") does not hide it. The same test as the database's
/// `has_card_number`, the backstop.
pub(crate) fn has_card_number(t: &str) -> bool {
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut groups = Vec::new();
        let mut group = Vec::new();
        loop {
            if i < chars.len() && chars[i].is_ascii_digit() {
                group.push(chars[i]);
                i += 1;
            } else if i + 1 < chars.len() && matches!(chars[i], ' ' | '-' | '.') && chars[i + 1].is_ascii_digit() {
                groups.push(std::mem::take(&mut group));
                i += 1;
            } else {
                break;
            }
        }
        groups.push(group);
        // a stretch stops growing past 19 digits, which keeps a 500-character message of short groups cheap
        let card = (0..groups.len()).any(|a| {
            groups[a..]
                .iter()
                .scan(Vec::new(), |digits, g| {
                    digits.extend_from_slice(g);
                    (digits.len() <= 19).then(|| digits.clone())
                })
                .any(|digits| digits.len() >= 13 && luhn(&digits))
        });
        if card {
            return true;
        }
    }
    false
}

fn luhn(digits: &[char]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, c)| {
            let d = c.to_digit(10).unwrap_or(0);
            if i % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { d }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// Characters that show nothing or reorder the text around them: zero-width spaces and joiners but the emoji joiner,
/// direction marks and overrides, the soft hyphen, the BOM, and the Hangul and Braille blanks. A nickname or message
/// holding one is refused, so no entry reads blank or reversed. `web/src/components/guestbook/rules.ts` holds the
/// same list.
const INVISIBLE: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{180E}', '\u{180E}'),
    ('\u{200B}', '\u{200C}'),
    ('\u{200E}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{2069}'),
    ('\u{2800}', '\u{2800}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
];

fn invisible(c: char) -> bool {
    INVISIBLE.iter().any(|(a, b)| (*a..=*b).contains(&c))
}

/// Text made only of what an emoji keeps, the zero-width joiner (U+200D) and variation selectors, shows nothing.
fn blank(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace() || c == '\u{200D}' || ('\u{FE00}'..='\u{FE0F}').contains(&c))
}

/// The trimmed nickname and message, or the refusal they deserve. A line break and a tab are the only control
/// characters a message keeps; a nickname keeps none.
fn validate(nickname: &str, body: &str) -> Result<(String, String), Refusal> {
    let nickname = nickname.trim();
    let body = body.replace("\r\n", "\n");
    let body = body.trim();
    let shown = |s: &str, max: usize| (1..=max).contains(&s.chars().count()) && !blank(s) && !s.chars().any(invisible);
    if !shown(nickname, NICKNAME_MAX)
        || !shown(body, BODY_MAX)
        || nickname.chars().any(char::is_control)
        || body.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(Refusal::BadGuestbookEntry);
    }
    if has_card_number(nickname) || has_card_number(body) {
        return Err(Refusal::CardNumber);
    }
    Ok((nickname.to_owned(), body.to_owned()))
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    before: Option<i64>,
}

pub(crate) async fn list(State(s): State<Arc<AppState>>, ClientIp(ip): ClientIp, Query(q): Query<ListQuery>) -> Result<Response, ApiError> {
    s.limits.lookup.check(ip)?;
    let mut entries: Vec<GuestbookEntry> = sqlx::query_as(
        "select id, nickname, body, created_at from guestbook
         where hidden_at is null and ($1::bigint is null or id < $1) order by id desc limit $2",
    )
    .bind(q.before)
    .bind(PAGE + 1)
    .fetch_all(&s.pool)
    .await?;
    let more = entries.len() as i64 > PAGE;
    entries.truncate(PAGE as usize);
    let site_key = turnstile::config(&s.pool).await?.keys().map(|(site, _)| site.to_owned());
    Ok(Json(GuestbookPage { entries, more, site_key }).into_response())
}

#[derive(Deserialize)]
pub(crate) struct NewEntry {
    pub(crate) nickname: String,
    pub(crate) body: String,
    pub(crate) token: String,
}

pub(crate) async fn post(State(s): State<Arc<AppState>>, ClientIp(ip): ClientIp, Json(b): Json<NewEntry>) -> Result<Response, ApiError> {
    // checked before the token: a refused form costs siteverify nothing, and a visitor fixes it without a new check
    let (nickname, body) = validate(&b.nickname, &b.body)?;
    s.limits.guestbook.check(ip)?;
    turnstile::require(&s, &b.token, ip, ACTION).await?;
    let entry: GuestbookEntry =
        sqlx::query_as("insert into guestbook (nickname, body) values ($1, $2) returning id, nickname, body, created_at")
            .bind(&nickname)
            .bind(&body)
            .fetch_one(&s.pool)
            .await?;
    println!("guestbook: entry {} posted from {ip}", entry.id);
    Ok(Json(entry).into_response())
}

#[cfg(test)]
#[path = "guestbook_tests.rs"]
mod tests;
