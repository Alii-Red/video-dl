//! Application layer — use cases orchestrating the domain, plus the ports
//! (traits) that infrastructure implements. Depends on `domain` only.

pub mod dto;
pub mod error;
pub mod events;
pub mod ports;
pub mod use_cases;
