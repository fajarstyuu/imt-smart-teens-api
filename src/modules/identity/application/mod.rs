//! Application facades own authorization and workflow ports; no SQL/Axum/environment.
mod schools;
mod sessions;
pub use schools::*;
pub use sessions::*;
