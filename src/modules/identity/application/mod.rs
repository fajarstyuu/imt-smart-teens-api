//! Application facades own authorization and workflow ports; no SQL/Axum/environment.
mod registration;
pub use registration::*;
mod schools;
mod sessions;
pub use schools::*;
pub use sessions::*;
