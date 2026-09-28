//! IPC layer (presentation) — the boundary with the frontend.
//!
//! Phase 2 defines the *contract*: serializable DTOs (with `ts-rs` TypeScript
//! generation), reusable command handlers over [`crate::state::AppState`], error
//! mapping, and event payloads. The thin `#[tauri::command]` wrappers and the
//! event-bus → `emit` bridge are added when the Tauri app shell is scaffolded
//! (Frontend phase). Keeping the handlers Tauri-free here makes them unit-testable
//! and keeps the runtime out of the core build.

pub mod commands;
pub mod dto;
pub mod error;
pub mod events;
