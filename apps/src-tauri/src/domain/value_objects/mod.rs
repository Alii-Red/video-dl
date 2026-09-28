//! Value objects — immutable, compared by value, validated at construction.

mod media_format;
mod media_url;
mod progress;
mod quality;
mod settings;

pub use media_format::MediaFormat;
pub use media_url::MediaUrl;
pub use progress::Progress;
pub use quality::Quality;
pub use settings::{Settings, WindowGeometry, WindowMode};
