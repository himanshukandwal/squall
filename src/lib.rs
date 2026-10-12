//! squall: an in-memory key-value store with Redis-style data types.

mod clock;
mod error;
mod expiry;
mod glob;
mod hashes;
mod keyspace;
mod lists;
mod sets;
mod store;
mod strings;
mod value;
mod zsets;

pub use clock::{Clock, ManualClock, SystemClock};
pub use error::Error;
pub use expiry::Ttl;
pub use store::Store;
pub use zsets::Bound;
