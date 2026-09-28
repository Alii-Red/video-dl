use crate::application::events::DomainEvent;

/// Port for publishing domain events onto the internal bus. Synchronous:
/// publishing is fire-and-forget from the caller's perspective.
pub trait EventPublisher: Send + Sync {
    fn publish(&self, event: DomainEvent);
}
