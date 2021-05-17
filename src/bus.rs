pub use tokio::sync::broadcast::error::RecvError as EventRecvError;
pub use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};
use tokio::sync::broadcast;

pub type EventReceiver<T> = BroadcastReceiver<T>;
pub(crate) type EventSender<T> = BroadcastSender<T>;

#[derive(Clone)]
pub struct EventBus<T: Clone> {
    tx: EventSender<T>,
}

impl<T: Clone> EventBus<T> {
    pub fn subscribe(&self) -> EventReceiver<T> {
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
