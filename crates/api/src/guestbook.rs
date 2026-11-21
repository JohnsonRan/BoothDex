//! The guestbook: anyone may post a message under a nickname after a Turnstile check. A message is final once posted
//! (the site asks the author to confirm first); every visitor only reads. An admin hides or deletes messages, and
//! closes one as completed or as not planned the way an issue is closed (admin/guestbook.rs). The client IP of a
//! post is logged, never stored.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::Refusal;
use crate::limit::{Check, ClientIp, Tier};
use crate::{ApiError, AppState, turnstile};

pub(crate) const NICKNAME_MAX: usize = 24;
pub(crate) const BODY_MAX: usize = 500;
pub(crate) const BODY_LINES: usize = 10;
const PAGE: i64 = 50;
/// The action the site's widget names; a token for another form does not pass here.
pub(crate) const ACTION: &str = "guestbook";

/// Where a message stands, as an issue does: open, or closed by an admin as completed or as not planned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub(crate) enum GuestbookStatus {
    Open,
    Completed,
    NotPlanned,
}

#[derive(Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub(crate) struct GuestbookEntry {
    pub(crate) id: i64,
    pub(crate) nickname: String,
    pub(crate) body: String,
    pub(crate) status: GuestbookStatus,
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

/// Characters that show nothing or reorder the text around them: zero-width spaces and the non-joiner, direction
/// marks, overrides and isolates, the soft hyphen, the BOM, the Hangul and Braille blanks, the annotation, object and
/// notation controls, and the unassigned default-ignorable code points around the specials and in plane 14 outside
/// `SHAPING`'s tags and selectors. A nickname or message holding one is refused, so no entry reads blank or reversed.
/// `web/src/components/guestbook/rules.test.ts` reads this list and `SHAPING` and holds the form's copy to them.
const INVISIBLE: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{180E}', '\u{180E}'),
    ('\u{200B}', '\u{200C}'),
    ('\u{200E}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{206F}'),
    ('\u{2800}', '\u{2800}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{FFF0}', '\u{FFFC}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0080}', '\u{E00FF}'),
    ('\u{E01F0}', '\u{E0FFF}'),
];

/// Characters that show nothing alone but shape the text beside them, so a field keeps them: the joiners of emoji
/// and of scripts, Khmer's inherent vowels, the variation selectors (Mongolian ones included) and the tags of a flag
/// emoji. A field made only of them and spaces is refused as blank.
const SHAPING: &[(char, char)] = &[
    ('\u{034F}', '\u{034F}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{180B}', '\u{180D}'),
    ('\u{180F}', '\u{180F}'),
    ('\u{200D}', '\u{200D}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{E0000}', '\u{E007F}'),
    ('\u{E0100}', '\u{E01EF}'),
];

fn within(set: &[(char, char)], c: char) -> bool {
    set.iter().any(|(a, b)| (*a..=*b).contains(&c))
}

fn blank(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace() || within(SHAPING, c))
}

/// The message as it is kept: each line break a line feed (the line and paragraph separators break a line as one
/// does), trimmed, and each run of blank lines one empty line, so a message cannot be a tall block of nothing.
/// `web/src/components/guestbook/rules.ts` (`normalBody`) does the same.
fn normal_body(body: &str) -> String {
    let body = body.replace("\r\n", "\n").replace(['\u{2028}', '\u{2029}'], "\n");
    let mut lines: Vec<&str> = Vec::new();
    for line in body.trim().split('\n') {
        if !line.chars().all(char::is_whitespace) {
            lines.push(line);
        } else if lines.last() != Some(&"") {
            lines.push("");
        }
    }
    lines.join("\n")
}

/// The trimmed nickname and the message as it is kept (`normal_body`), or the refusal they deserve. A line break and a
/// tab are the only control characters a message keeps, in at most `BODY_LINES` lines; a nickname keeps none, nor the
/// line and paragraph separators, which break a line as a line feed does.
fn validate(nickname: &str, body: &str) -> Result<(String, String), Refusal> {
    let nickname = nickname.trim();
    let body = normal_body(body);
    let shown = |s: &str, max: usize| (1..=max).contains(&s.chars().count()) && !blank(s) && !s.chars().any(|c| within(INVISIBLE, c));
    if !shown(nickname, NICKNAME_MAX)
        || !shown(&body, BODY_MAX)
        || body.split('\n').count() > BODY_LINES
        || nickname.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        || body.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(Refusal::BadGuestbookEntry);
    }
    Ok((nickname.to_owned(), body))
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    before: Option<i64>,
}

pub(crate) async fn list(State(s): State<Arc<AppState>>, ClientIp(ip): ClientIp, tier: Tier, Query(q): Query<ListQuery>) -> Result<Response, ApiError> {
    s.limits.lookup.check(ip, &tier).await?;
    let mut entries: Vec<GuestbookEntry> = sqlx::query_as(
        "select id, nickname, body, status, created_at from guestbook
         where hidden_at is null and ($1::bigint is null or id < $1) order by id desc limit $2",
    )
    .bind(q.before)
    .bind(PAGE + 1)
    .fetch_all(&s.pool)
    .await?;
    let more = entries.len() as i64 > PAGE;
    entries.truncate(PAGE as usize);
    let site_key = s.turnstile_config().await?.keys().map(|(site, _)| site.to_owned());
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
        sqlx::query_as("insert into guestbook (nickname, body) values ($1, $2) returning id, nickname, body, status, created_at")
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
