//! Video DL — core backend library.
//!
//! Clean Architecture (allégée) : `domain` (pur) ← `application` (use cases + ports)
//! ← `infrastructure` (implémentations). `state` est la composition root qui câble
//! le tout. Voir `docs/architecture/ADR-0001-architecture-globale.md`.

pub mod application;
pub mod desktop;
pub mod domain;
pub mod infrastructure;
pub mod ipc;
pub mod state;

pub use state::AppState;
