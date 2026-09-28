//! Task scheduler (Phase 5 of the ADR pipeline): pulls queued tasks and runs
//! them through the engine, bounded by a concurrency limit.

mod running;
mod task_scheduler;

pub use running::RunningTasks;
pub use task_scheduler::TaskScheduler;
