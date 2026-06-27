//! Identifier generation. UUIDv7 is time-ordered, which keeps index locality good.

use uuid::Uuid;

/// Generate a fresh time-ordered UUIDv7.
pub fn new_id() -> Uuid {
    Uuid::now_v7()
}
