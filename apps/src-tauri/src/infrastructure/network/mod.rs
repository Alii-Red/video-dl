//! Network / detection layer (Partie 4 of the ADR).
//!
//! Detection sources (all converge on `probe` → `MediaDetected`):
//!   1. manual import + HTTP probe + M3U8/MPD parsing
//!   2. JS injection (hook fetch/XHR) in the internal extraction window
//!   3. Opera GX extension → local WebSocket bridge (`ExtensionWsBridge`)
//!
//! (The MITM proxy originally planned for Phase 9 was abandoned — see ADR 4.5.)

mod classify;
mod dash;
mod extension_ws;
mod hls;
mod http_probe;
mod stub_probe;

pub use classify::classify;
pub use dash::{analyze_dash, DashInfo};
pub use extension_ws::{ExtensionWsBridge, ExtensionWsHandle};
pub use hls::{analyze_hls, HlsInfo};
pub use http_probe::HttpMediaProbe;
pub use stub_probe::StubMediaProbe;
