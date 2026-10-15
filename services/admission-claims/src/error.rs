//! One error body: `{"error":{"code":"<code>"}}`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub const MALFORMED: &str = "malformed";
pub const BAD_SIGNATURE: &str = "bad_signature";
pub const WRONG_ORIGIN: &str = "wrong_origin";
pub const WRONG_ACTION: &str = "wrong_action";
pub const REUSED_NONCE: &str = "reused_nonce";
pub const EXPIRED: &str = "expired";
pub const NO_ACCESS: &str = "no_access";
pub const PROVISIONAL: &str = "provisional";
pub const WINDOW: &str = "window";
pub const DEMO: &str = "demo";
pub const RATE_LIMITED: &str = "rate_limited";
pub const UNAVAILABLE: &str = "unavailable";
pub const INVALID: &str = "invalid";
pub const RESERVED: &str = "reserved";
pub const TAKEN: &str = "taken";
pub const HAS_NAME: &str = "has_name";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fail {
    pub status: StatusCode,
    pub code: &'static str,
}

impl Fail {
    pub const fn new(status: StatusCode, code: &'static str) -> Self {
        Self { status, code }
    }

    pub const fn invalid() -> Self {
        Self::new(StatusCode::BAD_REQUEST, INVALID)
    }

    pub const fn reserved() -> Self {
        Self::new(StatusCode::FORBIDDEN, RESERVED)
    }

    pub const fn taken() -> Self {
        Self::new(StatusCode::CONFLICT, TAKEN)
    }

    pub const fn malformed() -> Self {
        Self::new(StatusCode::BAD_REQUEST, MALFORMED)
    }

    pub const fn bad_signature() -> Self {
        Self::new(StatusCode::BAD_REQUEST, BAD_SIGNATURE)
    }

    pub const fn wrong_origin() -> Self {
        Self::new(StatusCode::BAD_REQUEST, WRONG_ORIGIN)
    }

    pub const fn wrong_action() -> Self {
        Self::new(StatusCode::BAD_REQUEST, WRONG_ACTION)
    }

    pub const fn reused_nonce() -> Self {
        Self::new(StatusCode::CONFLICT, REUSED_NONCE)
    }

    pub const fn expired() -> Self {
        Self::new(StatusCode::BAD_REQUEST, EXPIRED)
    }

    pub const fn no_access() -> Self {
        Self::new(StatusCode::FORBIDDEN, NO_ACCESS)
    }

    pub const fn provisional() -> Self {
        Self::new(StatusCode::FORBIDDEN, PROVISIONAL)
    }

    pub const fn window() -> Self {
        Self::new(StatusCode::FORBIDDEN, WINDOW)
    }

    pub const fn demo() -> Self {
        Self::new(StatusCode::FORBIDDEN, DEMO)
    }

    pub const fn rate_limited() -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, RATE_LIMITED)
    }

    pub const fn unavailable() -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, UNAVAILABLE)
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": { "code": self.code } }));
        (self.status, body).into_response()
    }
}
