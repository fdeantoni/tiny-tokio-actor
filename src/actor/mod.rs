pub(crate) mod handler;
pub mod runner;

use thiserror::Error;
use async_trait::async_trait;
use uuid::Uuid;

use crate::system::{ActorSystem, SystemEvent};

pub struct ActorContext<E: SystemEvent> {
    pub id: Uuid,
    pub system: ActorSystem<E>
}

pub trait Message: Clone + Send + Sync + 'static {
    type Response: Send + Sync + 'static;
}

#[async_trait]
pub trait Handler<M: Message, E: SystemEvent>: Send + Sync {
    async fn handle(&mut self, msg: M, ctx: &mut ActorContext<E>) -> M::Response;
}

pub trait Actor: Clone + Send + Sync + 'static {}


#[derive(Clone)]
pub struct ActorRef<A: Actor, E: SystemEvent> {
    id: Uuid,
    sender: handler::HandlerRef<A, E>
}

impl<A: Actor, E: SystemEvent> ActorRef<A, E> {

    pub fn get_id(&self) -> &Uuid {
        &self.id
    }

    pub fn tell<M>(&mut self, msg: M) -> Result<(), ActorError>
    where
        M: Message,
        A: Handler<M, E>
    {
        self.sender.tell(msg)
    }

    pub async fn ask<M>(&mut self, msg: M) -> Result<M::Response, ActorError>
    where
        M: Message,
        A: Handler<M, E>
    {
        self.sender.ask(msg).await
    }

    pub(crate) fn get_any(&self) -> AnyActorRef {
        AnyActorRef {
            id: self.id,
            sender: self.sender.clone().into()
        }
    }

    pub fn new(id: Uuid, sender: handler::MailboxSender<A, E>) -> Self {
        let handler = handler::HandlerRef::new(sender);
        ActorRef {
            id,
            sender: handler
        }
    }
}

#[derive(Clone)]
pub(crate) struct AnyActorRef {
    id: Uuid,
    sender: handler::AnyHandlerRef
}

impl AnyActorRef {

    pub fn get_ref<A: Actor, E: SystemEvent>(&self) -> ActorRef<A, E> {
        let handler: handler::HandlerRef<A, E> = self.sender.clone().into();
            ActorRef {
                id: self.id,
                sender: handler
            }
    }
}

#[derive(Error, Debug)]
pub enum ActorError {

    #[error("Actor runtime error")]
    Runtime(String)
}
