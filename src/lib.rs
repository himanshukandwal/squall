//! squall: an in-memory key-value store with Redis-style data types.

mod error;
mod glob;
mod hashes;
mod keyspace;
mod lists;
mod sets;
mod store;
mod strings;
mod value;
mod zsets;

pub use error::Error;
pub use store::Store;
