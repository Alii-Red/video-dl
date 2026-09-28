//! Application-level DTOs.
//!
//! Phase 1 use cases pass domain types directly (single process, no IPC yet).
//! IPC-facing DTOs — with `ts-rs` derives for TypeScript generation — are added
//! in Phase 2 under the `ipc` layer, mapped from these domain types.
