//! Domain layer — pure business rules, no I/O, no async.
//!
//! If Tauri, SQLite and the network were removed, this module would still compile.

pub mod entities;
pub mod errors;
pub mod services;
pub mod value_objects;
