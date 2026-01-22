use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use tokio::sync::{mpsc, oneshot};

use crate::{
    actor::{ActorContext, Handler, Message},
    system::SystemEvent,
};

use super::Actor;

/// Internal trait for message handling with dynamic dispatch.
/// Uses boxed future for object safety.
pub trait MessageHandler<E: SystemEvent, A: Actor<E>>: Send + Sync {
    fn handle<'a>(
        &'a mut self,
        actor: &'a mut A,
        ctx: &'a mut ActorContext<E>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

pub(crate) struct ActorMessage<M, E, A>
where
    M: Message,
    E: SystemEvent,
    A: Handler<E, M>,
{
    payload: M,
    rsvp: Option<oneshot::Sender<M::Response>>,
    _phantom_actor: PhantomData<A>,
    _phantom_event: PhantomData<E>,
}

impl<M, E, A> MessageHandler<E, A> for ActorMessage<M, E, A>
where
    M: Message,
    E: SystemEvent,
    A: Handler<E, M>,
{
    fn handle<'a>(
        &'a mut self,
        actor: &'a mut A,
        ctx: &'a mut ActorContext<E>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let result = actor.handle(self.payload.clone(), ctx).await;

            if let Some(rsvp) = self.rsvp.take() {
                rsvp.send(result).unwrap_or_else(|_failed| {
                    log::error!("Failed to send back response!");
                })
            }
        })
    }
}

impl<M, E, A> ActorMessage<M, E, A>
where
    M: Message,
    E: SystemEvent,
    A: Handler<E, M>,
{
    pub fn new(msg: M, rsvp: Option<oneshot::Sender<M::Response>>) -> Self {
        ActorMessage {
            payload: msg,
            rsvp,
            _phantom_actor: PhantomData,
            _phantom_event: PhantomData,
        }
    }
}

pub type BoxedMessageHandler<E, A> = Box<dyn MessageHandler<E, A>>;

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

#[cfg(test)]
mod tests {
    #![expect(dead_code)]
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
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let mut actor = MyActor { counter: 0 };
        let msg = MyMessage("Hello World!".to_string());
        let (sender, mut receiver): (
            MailboxSender<MyMessage, MyActor>,
            MailboxReceiver<MyMessage, MyActor>,
        ) = ActorMailbox::create();

        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let path = ActorPath::from("/test");
        let mut ctx = ActorContext { path, system };
        tokio::spawn(async move {
            while let Some(mut msg) = receiver.recv().await {
                msg.handle(&mut actor, &mut ctx).await;
            }
        });

        let message = ActorMessage::new(msg, None);
        sender.send(Box::new(message)).ok();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    #[tokio::test]
    async fn actor_ask() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let mut actor = MyActor { counter: 0 };
        let msg = MyMessage("Hello World!".to_string());
        let (sender, mut receiver): (
            MailboxSender<MyMessage, MyActor>,
            MailboxReceiver<MyMessage, MyActor>,
        ) = ActorMailbox::create();

        let bus = EventBus::<MyMessage>::new(1000);
        let system = ActorSystem::new("test", bus);
        let path = ActorPath::from("/test");
        let mut ctx = ActorContext { path, system };
        tokio::spawn(async move {
            while let Some(mut msg) = receiver.recv().await {
                msg.handle(&mut actor, &mut ctx).await;
            }
        });

        let (response_sender, response_receiver) = oneshot::channel();
        let message = ActorMessage::new(msg, Some(response_sender));
        sender.send(Box::new(message)).ok();
        let result = response_receiver.await.unwrap();
        assert_eq!(result, 1);
    }
}
