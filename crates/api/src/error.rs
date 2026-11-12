//! What a handler answers when it does not succeed: a refusal the caller can act on, a rate limit, or a failure
//! inside the API. Handlers return `Result<Response, ApiError>` and leave with `?`.

use std::borrow::Cow;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::limit::RateLimited;

/// Every refusal the API answers with. Its name in snake_case is the `code` of the error body, which the site
/// words in the visitor's language (web/src/lib/messages.ts, web/src/components/admin/messages.ts): the generated
/// `Refusal` type of web/src/lib/api-types.ts fails the site's type check when a code it names is not listed here.
/// The message is for other clients and for a browser still running a script that matched on it.
#[derive(Clone, Copy, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub(crate) enum Refusal {
    SignInRequired,
    AdminsOnly,
    BadCredentials,
    TooManyAttempts,
    AccountDisabled,
    RegistrationClosed,
    DisplayNameRequired,
    BadUsername,
    BadEmail,
    PasswordTooShort,
    NameTaken,
    PasswordUnchanged,
    WrongCurrentPassword,
    WrongPassword,
    AdminCannotDelete,
    /// answered with the reason `account::validate_preferences` gives
    BadPreferences,
    UnknownItem,
    BadSavedSearch,
    SavedSearchLimit,
    NoSuchUser,
    AdminCannotBeDisabled,
    NoSuchJob,
    BadJob,
    JobAlreadyActive,
    JobNotActive,
    NoSuchSchedule,
    BadSchedule,
    BadAiSettings,
    AiNotConfigured,
    /// answered with what the provider said, for the admin who runs that connection
    ProviderFailed,
}

impl Refusal {
    fn parts(self) -> (StatusCode, &'static str) {
        use StatusCode as S;
        match self {
            Self::SignInRequired => (S::UNAUTHORIZED, "Sign in required"),
            Self::AdminsOnly => (S::FORBIDDEN, "Admins only"),
            Self::BadCredentials => (S::UNAUTHORIZED, "Incorrect username or password."),
            Self::TooManyAttempts => (S::TOO_MANY_REQUESTS, "Too many attempts. Try again later."),
            Self::AccountDisabled => (S::FORBIDDEN, "This account is disabled."),
            Self::RegistrationClosed => (S::FORBIDDEN, "Registration is closed."),
            Self::DisplayNameRequired => (S::BAD_REQUEST, "Display name is required."),
            Self::BadUsername => (S::BAD_REQUEST, "Username must be 3-24 letters, digits or underscores."),
            Self::BadEmail => (S::BAD_REQUEST, "Invalid email."),
            Self::PasswordTooShort => (S::BAD_REQUEST, "Your password must be at least 10 characters."),
            Self::NameTaken => (S::CONFLICT, "Username or email is already taken."),
            Self::PasswordUnchanged => (S::BAD_REQUEST, "Your new password must be different from your current password."),
            Self::WrongCurrentPassword => (S::FORBIDDEN, "Current password is incorrect."),
            Self::WrongPassword => (S::FORBIDDEN, "Password is incorrect."),
            Self::AdminCannotDelete => (S::CONFLICT, "Admins can't delete their account."),
            Self::BadPreferences => (S::BAD_REQUEST, "Invalid preferences."),
            Self::UnknownItem => (S::NOT_FOUND, "Unknown item."),
            Self::BadSavedSearch => (S::BAD_REQUEST, "Invalid name or query."),
            Self::SavedSearchLimit => (S::CONFLICT, "Saved search limit reached."),
            Self::NoSuchUser => (S::NOT_FOUND, "No such user"),
            Self::AdminCannotBeDisabled => (S::CONFLICT, "Admins can't be disabled."),
            Self::NoSuchJob => (S::NOT_FOUND, "No such job"),
            Self::BadJob => (S::BAD_REQUEST, "Unknown kind or invalid params"),
            Self::JobAlreadyActive => (S::CONFLICT, "A job of this kind is already queued or running"),
            Self::JobNotActive => (S::CONFLICT, "Job is not queued or running"),
            Self::NoSuchSchedule => (S::NOT_FOUND, "No such schedule"),
            Self::BadSchedule => (S::BAD_REQUEST, "Interval must be 5–10080 minutes; params must be an object"),
            Self::BadAiSettings => (
                S::BAD_REQUEST,
                "Base URL must start with http(s)://; model ≤ 100 chars; key ≤ 500 chars; reasoning effort: none, minimal, low, medium, high or xhigh",
            ),
            Self::AiNotConfigured => (S::BAD_REQUEST, "Set the base URL and API key first"),
            Self::ProviderFailed => (S::BAD_GATEWAY, "The AI provider failed."),
        }
    }

    /// This refusal with a message of its own in place of the listed one.
    pub(crate) fn with(self, message: impl Into<Cow<'static, str>>) -> ApiError {
        ApiError::Refused(self, message.into())
    }
}

pub(crate) enum ApiError {
    Refused(Refusal, Cow<'static, str>),
    Limited(RateLimited),
    /// Logged here; the caller is told nothing of the cause.
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Refused(refusal, message) => (refusal.parts().0, Json(json!({ "ok": false, "code": refusal, "error": message }))).into_response(),
            Self::Limited(limited) => limited.into_response(),
            Self::Internal(cause) => {
                eprintln!("error: {cause:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "ok": false, "error": "internal error" }))).into_response()
            }
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self::Internal(e.into())
    }
}

impl From<Refusal> for ApiError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal, refusal.parts().1.into())
    }
}

impl From<RateLimited> for ApiError {
    fn from(limited: RateLimited) -> Self {
        Self::Limited(limited)
    }
}
