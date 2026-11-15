//! Application facades own authorization and workflow ports; no SQL/Axum/environment.
mod content;
pub use content::{authorize_content, content_actor};
mod registration;
pub use registration::*;
mod schools;
mod sessions;
pub use schools::*;
pub use sessions::*;
