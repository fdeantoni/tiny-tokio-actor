pub use tokio::sync::broadcast::error::RecvError as EventRecvError;
pub use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use tokio::sync::broadcast;

pub type EventConsumer<T> = BroadcastReceiver<T>;
pub(crate) type EventProducer<T> = BroadcastSender<T>;

#[derive(Clone)]
pub struct EventBus<T: Clone> {
    tx: EventProducer<T>,
}

impl<T: Clone> EventBus<T> {
    pub fn subscribe(&self) -> EventConsumer<T> {
        self.tx.subscribe()
    }

    pub fn send(&self, event: T) -> Result<usize, SendError<T>> {
        self.tx.send(event)
    }

    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        EventBus { tx }
    }
}
