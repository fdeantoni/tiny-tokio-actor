pub(crate) mod handler;
pub(crate) mod runner;

mod path;
pub use path::ActorPath;

use thiserror::Error;
use async_trait::async_trait;

use crate::system::{ActorSystem, SystemEvent};

pub struct ActorContext<E: SystemEvent> {
    pub path: ActorPath,
    pub system: ActorSystem<E>
}

impl<E: SystemEvent> ActorContext<E> {

    pub async fn create_child<A: Actor>(&self, name: &str, actor: A) -> Result<ActorRef<A, E>, ActorError> {
        let path = self.path.clone() / name;
        self.system.create_actor_path(path, actor).await
    }

    pub async fn get_child<A: Actor>(&self, name: &str) -> Option<ActorRef<A, E>> {
        let path = self.path.clone() / name;
        self.system.get_actor(&path).await
    }
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
    path: ActorPath,
    sender: handler::HandlerRef<A, E>
}

impl<A: Actor, E: SystemEvent> ActorRef<A, E> {

    pub fn get_path(&self) -> &ActorPath {
        &self.path
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

    pub fn new(path: ActorPath, sender: handler::MailboxSender<A, E>) -> Self {
        let handler = handler::HandlerRef::new(sender);
        ActorRef {
            path,
            sender: handler
        }
    }
}

#[derive(Error, Debug)]
pub enum ActorError {

    #[error("Actor creation failed")]
    Create(String),

    #[error("Actor runtime error")]
    Runtime(String)
}
