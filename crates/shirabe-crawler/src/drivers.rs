//! Concrete [`PageDriver`] implementations.
//!
//! - [`mock`] — in-memory, for tests and offline dev.
//! - [`shirabe`] — the canonical backend: drives a running shirabe debug server
//!   over HTTP.

pub mod mock;
#[cfg(feature = "shirabe-driver")]
pub mod shirabe;

pub use mock::MockDriver;
#[cfg(feature = "shirabe-driver")]
pub use shirabe::{ShirabeDriver, ShirabeDriverConfig};
