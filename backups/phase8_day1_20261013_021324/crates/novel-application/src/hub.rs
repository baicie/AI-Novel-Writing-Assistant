use tokio::sync::broadcast;

use crate::dto::GenerationEvent;

#[derive(Clone)]
pub struct EventHub {
    sender: broadcast::Sender<GenerationEvent>,
}

impl EventHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    pub fn emit(&self, event: GenerationEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GenerationEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}
