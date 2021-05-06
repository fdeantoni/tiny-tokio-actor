use std::{any::Any, marker::PhantomData};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use crate::{actor::{ActorContext, Handler, Message}, system::SystemEvent};

use super::{Actor, ActorError};

#[async_trait]
pub trait MessageHandler<A: Actor, E: SystemEvent>: Send + Sync {

    async fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<E>);
}

struct ActorMessage<M, A, E>
where
    M: Message,
    E: SystemEvent,
    A: Actor + Handler<M, E>,
{
    payload: M,
    rsvp: Option<oneshot::Sender<M::Response>>,
    _phantom_actor: PhantomData<A>,
    _phantom_event: PhantomData<E>
}

#[async_trait]
impl<M, A, E> MessageHandler<A, E> for ActorMessage<M, A, E>
where
    M: Message,
    E: SystemEvent,
    A: Actor + Handler<M, E>,
{
    async fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<E>) {
        self.process(actor, ctx).await
    }
}

impl<M, A, E> ActorMessage<M, A, E>
where
    M: Message,
    E: SystemEvent,
    A: Actor + Handler<M, E>,
{

    async fn process(&mut self, actor: &mut A, ctx: &mut ActorContext<E>) {

        let result = actor.handle(self.payload.clone(), ctx).await;

        if let Some(rsvp) = std::mem::replace(&mut self.rsvp, None) {
            rsvp.send(result).unwrap_or_else(|failed| {
                log::error!("Failed to send back result '{:?}'", failed);
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

pub type MailboxReceiver<A, E> = mpsc::UnboundedReceiver<BoxedMessageHandler<A, E>>;
pub type MailboxSender<A, E> = mpsc::UnboundedSender<BoxedMessageHandler<A, E>>;

pub struct ActorMailbox<A: Actor, E: SystemEvent> {
    _phantom_actor: PhantomData<A>,
    _phantom_event: PhantomData<E>
}

impl<A: Actor, E: SystemEvent> ActorMailbox<A, E> {

    pub fn create() -> (MailboxSender<A, E>, MailboxReceiver<A, E>) {
        mpsc::unbounded_channel()
    }
}

pub type BoxedMessageHandler<A, E> = Box<dyn MessageHandler<A, E>>;
pub type AnyMessageHandler = Box<dyn Any + Sync + Send>;

#[derive(Clone)]
pub struct HandlerRef<A: Actor, E: SystemEvent> {
    sender: mpsc::UnboundedSender<BoxedMessageHandler<A, E>>
}

#[derive(Clone)]
pub struct AnyHandlerRef {
    pub sender: mpsc::UnboundedSender<AnyMessageHandler>
}

impl<A: Actor, E: SystemEvent> From<AnyHandlerRef> for HandlerRef<A, E> {
    fn from(handler: AnyHandlerRef) -> Self {
        HandlerRef {
            sender: unsafe { std::mem::transmute(handler.sender) }
        }
    }
}

impl<A: Actor, E: SystemEvent> From<HandlerRef<A, E>> for AnyHandlerRef {
    fn from(handler: HandlerRef<A, E>) -> Self {
        AnyHandlerRef {
            sender: unsafe { std::mem::transmute(handler.sender) }
        }
    }
}

impl<A: Actor, E: SystemEvent> HandlerRef<A, E> {

    pub(crate) fn new(sender: mpsc::UnboundedSender<BoxedMessageHandler<A, E>>) -> Self {
        HandlerRef {
            sender
        }
    }

    pub fn tell<M>(&mut self, msg: M) -> Result<(), ActorError>
    where
        M: Message,
        A: Handler<M, E>
    {
        let message = ActorMessage::<M, A, E>::new(msg, None);
        if let Err(error) = self.sender.send(Box::new(message)) {
            log::error!("Failed to tell message! {}", error.to_string());
            Err(ActorError::Runtime(error.to_string()))
        } else {
            Ok(())
        }
    }

    pub async fn ask<M>(&mut self, msg: M) -> Result<M::Response, ActorError>
    where
        M: Message,
        A: Handler<M, E>
    {
        let (response_sender, response_receiver) = oneshot::channel();
        let message = ActorMessage::<M, A, E>::new(msg, Some(response_sender));
        if let Err(error) = self.sender.send(Box::new(message)) {
            log::error!("Failed to ask message! {}", error.to_string());
            Err(ActorError::Runtime(error.to_string()))
        } else {
            response_receiver.await.map_err(|error| {
                ActorError::Runtime(error.to_string())
            })
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{bus::EventBus, system::ActorSystem};

    use super::*;
    use uuid::Uuid;

    #[derive(Clone)]
    struct MyActor {
        counter: usize
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

    impl Actor for MyActor {}

    #[tokio::test]
    async fn actor_tell() {

        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let mut actor = MyActor { counter: 0 };
        let msg = MyMessage("Hello World!".to_string());
        let (sender, mut receiver): (MailboxSender<MyActor, MyMessage>, MailboxReceiver<MyActor, MyMessage>) = ActorMailbox::create();
        let mut actor_ref = HandlerRef {
            sender
        };
        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let id =  Uuid::new_v4();
        let mut ctx = ActorContext { id, system };
        tokio::spawn( async move {
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
        let (sender, mut receiver): (MailboxSender<MyActor, MyMessage>, MailboxReceiver<MyActor, MyMessage>) = ActorMailbox::create();
        let mut actor_ref = HandlerRef {
            sender
        };
        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let id =  Uuid::new_v4();
        let mut ctx = ActorContext { id, system };
        tokio::spawn( async move {
            while let Some(mut msg) = receiver.recv().await {
                msg.handle(&mut actor, &mut ctx).await;
            }
        });

        let result = actor_ref.ask(msg).await.unwrap();
        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn ref_transmute() {

        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let mut actor = MyActor { counter: 0 };
        let msg = MyMessage("Hello World!".to_string());
        let (sender, mut receiver): (MailboxSender<MyActor, MyMessage>, MailboxReceiver<MyActor, MyMessage>) = ActorMailbox::create();
        let actor_ref = HandlerRef {
            sender
        };
        let any_ref: AnyHandlerRef = actor_ref.into();
        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let id =  Uuid::new_v4();
        let mut ctx = ActorContext { id, system };
        tokio::spawn( async move {
            while let Some(mut msg) = receiver.recv().await {
                msg.handle(&mut actor, &mut ctx).await;
            }
        });

        let mut check: HandlerRef<MyActor, MyMessage> = any_ref.into();
        check.tell(msg).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

}