//! Identity HTTP contracts map explicitly to core application types.
mod registration;
pub use registration::*;
mod dto;
mod schools;
mod sessions;
pub use schools::*;
pub use sessions::*;
