use std::marker::PhantomData;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use crate::{
    actor::{ActorContext, Handler, Message},
    system::SystemEvent,
};

use super::{Actor, ActorError};

#[async_trait]
pub trait MessageHandler<E: SystemEvent, A: Actor<E>>: Send + Sync {
    async fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<E>);
}

struct ActorMessage<M, E, A>
where
    M: Message,
    E: SystemEvent,
    A: Actor<E> + Handler<E, M>,
{
    payload: M,
    rsvp: Option<oneshot::Sender<M::Response>>,
    _phantom_actor: PhantomData<A>,
    _phantom_event: PhantomData<E>,
}

#[async_trait]
impl<M, E, A> MessageHandler<E, A> for ActorMessage<M, E, A>
where
    M: Message,
    E: SystemEvent,
    A: Actor<E> + Handler<E, M>,
{
    async fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<E>) {
        self.process(actor, ctx).await
    }
}

impl<M, E, A> ActorMessage<M, E, A>
where
    M: Message,
    E: SystemEvent,
    A: Actor<E> + Handler<E, M>,
{
    async fn process(&mut self, actor: &mut A, ctx: &mut ActorContext<E>) {
        let result = actor.handle(self.payload.clone(), ctx).await;

        if let Some(rsvp) = std::mem::replace(&mut self.rsvp, None) {
            rsvp.send(result).unwrap_or_else(|_failed| {
                log::error!("Failed to send back response!");
            })
        }
    }

    pub fn new(msg: M, rsvp: Option<oneshot::Sender<M::Response>>) -> Self {
        ActorMessage {
            payload: msg,
            rsvp,
            _phantom_actor: PhantomData,
            _phantom_event: PhantomData,
        }
    }
}

pub type MailboxReceiver<E, A> = mpsc::UnboundedReceiver<BoxedMessageHandler<E, A>>;
pub type MailboxSender<E, A> = mpsc::UnboundedSender<BoxedMessageHandler<E, A>>;

pub struct ActorMailbox<E: SystemEvent, A: Actor<E>> {
    _phantom_actor: PhantomData<A>,
    _phantom_event: PhantomData<E>,
}

impl<E: SystemEvent, A: Actor<E>> ActorMailbox<E, A> {
    pub fn create() -> (MailboxSender<E, A>, MailboxReceiver<E, A>) {
        mpsc::unbounded_channel()
    }
}

pub type BoxedMessageHandler<E, A> = Box<dyn MessageHandler<E, A>>;

pub struct HandlerRef<E: SystemEvent, A: Actor<E>> {
    sender: mpsc::UnboundedSender<BoxedMessageHandler<E, A>>,
}

impl<E: SystemEvent, A: Actor<E>> Clone for HandlerRef<E, A> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}

impl<E: SystemEvent, A: Actor<E>> HandlerRef<E, A> {
    pub(crate) fn new(sender: mpsc::UnboundedSender<BoxedMessageHandler<E, A>>) -> Self {
        HandlerRef { sender }
    }

    pub fn tell<M>(&self, msg: M) -> Result<(), ActorError>
    where
        M: Message,
        A: Handler<E, M>,
    {
        let message = ActorMessage::<M, E, A>::new(msg, None);
        if let Err(error) = self.sender.send(Box::new(message)) {
            log::error!("Failed to tell message! {}", error.to_string());
            Err(ActorError::SendError(error.to_string()))
        } else {
            Ok(())
        }
    }

    pub async fn ask<M>(&self, msg: M) -> Result<M::Response, ActorError>
    where
        M: Message,
        A: Handler<E, M>,
    {
        let (response_sender, response_receiver) = oneshot::channel();
        let message = ActorMessage::<M, E, A>::new(msg, Some(response_sender));
        if let Err(error) = self.sender.send(Box::new(message)) {
            log::error!("Failed to ask message! {}", error.to_string());
            Err(ActorError::SendError(error.to_string()))
        } else {
            response_receiver
                .await
                .map_err(|error| ActorError::SendError(error.to_string()))
        }
    }

    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

#[cfg(test)]
mod tests {

    use crate::{bus::EventBus, system::ActorSystem, ActorPath};

    use super::*;

    #[derive(Default, Clone)]
    struct MyActor {
        counter: usize,
    }

    #[derive(Debug, Clone)]
    struct MyMessage(String);

    impl Message for MyMessage {
        type Response = usize;
    }

    impl SystemEvent for MyMessage {}

    #[async_trait]
    impl Handler<MyMessage, MyMessage> for MyActor {
        async fn handle(&mut self, msg: MyMessage, _ctx: &mut ActorContext<MyMessage>) -> usize {
            log::debug!("received message! {:?}", &msg);
            self.counter += 1;
            log::debug!("counter is now {}", &self.counter);
            self.counter
        }
    }

    impl Actor<MyMessage> for MyActor {}

    #[tokio::test]
    async fn actor_tell() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let mut actor = MyActor { counter: 0 };
        let msg = MyMessage("Hello World!".to_string());
        let (sender, mut receiver): (
            MailboxSender<MyMessage, MyActor>,
            MailboxReceiver<MyMessage, MyActor>,
        ) = ActorMailbox::create();
        let actor_ref = HandlerRef { sender };
        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let path = ActorPath::from("/test");
        let mut ctx = ActorContext { path, system };
        tokio::spawn(async move {
            while let Some(mut msg) = receiver.recv().await {
                msg.handle(&mut actor, &mut ctx).await;
            }
        });

        actor_ref.tell(msg).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    #[tokio::test]
    async fn actor_ask() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let mut actor = MyActor { counter: 0 };
        let msg = MyMessage("Hello World!".to_string());
        let (sender, mut receiver): (
            MailboxSender<MyMessage, MyActor>,
            MailboxReceiver<MyMessage, MyActor>,
        ) = ActorMailbox::create();
        let actor_ref = HandlerRef { sender };
        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let path = ActorPath::from("/test");
        let mut ctx = ActorContext { path, system };
        tokio::spawn(async move {
            while let Some(mut msg) = receiver.recv().await {
                msg.handle(&mut actor, &mut ctx).await;
            }
        });

        let result = actor_ref.ask(msg).await.unwrap();
        assert_eq!(result, 1);
    }
}
