use tokio::sync::broadcast;

use crate::application::events::DomainEvent;
use crate::application::ports::EventPublisher;

/// In-process event bus backed by a tokio broadcast channel. Producers publish;
/// consumers `subscribe`. In Phase 2 an IPC bridge subscribes here and forwards
/// events to the frontend via Tauri `emit`.
pub struct InMemoryEventBus {
    sender: broadcast::Sender<DomainEvent>,
}

impl InMemoryEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.sender.subscribe()
    }
}

impl EventPublisher for InMemoryEventBus {
    fn publish(&self, event: DomainEvent) {
        // With no active subscribers, broadcast returns Err — expected, ignore it.
        let _ = self.sender.send(event);
    }
}
