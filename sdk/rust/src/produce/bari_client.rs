//! SPEC §13.1 waypoint POST and path GET.
//! The producer builds the request. A rig or a later HTTP adapter sends it.
//! The adapter does not follow a redirect. The walk does not call this trait.

use std::sync::Arc;

/// `POST {bari-url}/waypoint`. Body is the 32 waypoint bytes.
#[derive(Clone, Debug)]
pub struct WaypointPost {
    pub url: String,
    pub body: [u8; 32],
    pub content_type: &'static str,
    /// `X-Bari-Hashcash-Nonce`, shortest decimal.
    pub nonce: String,
    /// `X-Bari-Hashcash-Header`, 160 lowercase hex digits, byte 0 first.
    pub fee_header_hex: String,
}

/// `GET {bari-url}/path/{leaf}`.
#[derive(Clone, Debug)]
pub struct PathGet {
    pub url: String,
    pub leaf_hex: String,
}

/// What the transport returned. `NoStatus` is a transitory send failure with no status line.
#[derive(Clone, Debug)]
pub enum BariResponse {
    NoStatus,
    Http { status: u16, body: Vec<u8> },
}

impl BariResponse {
    pub fn status(&self) -> Option<u16> {
        match self {
            BariResponse::NoStatus => None,
            BariResponse::Http { status, .. } => Some(*status),
        }
    }

    /// SPEC §13.1. No status line, or 502, 503, or 504. Not a Bari answer.
    pub fn is_transport_failure(&self) -> bool {
        match self {
            BariResponse::NoStatus => true,
            BariResponse::Http { status, .. } => matches!(*status, 502 | 503 | 504),
        }
    }
}

/// Sends one POST or one GET. Implementations must not be called on the walk thread.
pub trait BariClient: Send + Sync {
    fn post_waypoint(&self, post: &WaypointPost) -> BariResponse;
    fn get_path(&self, ask: &PathGet) -> BariResponse;
}

pub type Client = Arc<dyn BariClient>;
