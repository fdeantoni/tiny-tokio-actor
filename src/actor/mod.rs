pub(crate) mod handler;
pub(crate) mod runner;

mod path;
pub use path::ActorPath;

use thiserror::Error;
use async_trait::async_trait;

use crate::system::{ActorSystem, SystemEvent};

/// The actor context gives a running actor access to its path, as well as the system that
/// is running it.
pub struct ActorContext<E: SystemEvent> {
    pub path: ActorPath,
    pub system: ActorSystem<E>
}

impl<E: SystemEvent> ActorContext<E> {

    /// Create a child actor under this actor.
    pub async fn create_child<A: Actor<E>>(&self, name: &str, actor: A) -> Result<ActorRef<A, E>, ActorError> {
        let path = self.path.clone() / name;
        self.system.create_actor_path(path, actor).await
    }

    /// Retrieve a child actor running under this actor.
    pub async fn get_child<A: Actor<E>>(&self, name: &str) -> Option<ActorRef<A, E>> {
        let path = self.path.clone() / name;
        self.system.get_actor(&path).await
    }
}

/// Defines what an actor will receive as its message, and with what it should respond.
pub trait Message: Clone + Send + Sync + 'static {
    /// response an actor should give when it receives this message. If no response is
    /// required, use `()`.
    type Response: Send + Sync + 'static;
}

/// Defines what the actor does with a message.
#[async_trait]
pub trait Handler<M: Message, E: SystemEvent>: Send + Sync {
    async fn handle(&mut self, msg: M, ctx: &mut ActorContext<E>) -> M::Response;
}

/// Basic trait for actors. Allows you to define tasks that should be run before
/// actor startup, and tasks that should be run after the  actor is stopped.
#[async_trait]
pub trait Actor<E: SystemEvent>: Clone + Send + Sync + 'static {

    /// Override this function if you like to perform initialization of the actor
    async fn pre_start(&mut self, _ctx: &mut ActorContext<E>) {}

    /// Override this function if you like to perform work when the actor is stopped
    async fn post_stop(&mut self, _ctx: &mut ActorContext<E>) {}
}

/// A clonable actor reference. It basically holds a Sender that can send messages
/// to the mailbox (receiver) of the actor.
#[derive(Clone)]
pub struct ActorRef<A: Actor<E>, E: SystemEvent> {
    path: ActorPath,
    sender: handler::HandlerRef<A, E>
}

impl<A: Actor<E>, E: SystemEvent> ActorRef<A, E> {

    /// Get the path of this actor
    pub fn get_path(&self) -> &ActorPath {
        &self.path
    }

    /// Fire and forget sending of messages to this actor.
    pub fn tell<M>(&mut self, msg: M) -> Result<(), ActorError>
    where
        M: Message,
        A: Handler<M, E>
    {
        self.sender.tell(msg)
    }

    /// Send a message to an actor, expecting a response.
    pub async fn ask<M>(&mut self, msg: M) -> Result<M::Response, ActorError>
    where
        M: Message,
        A: Handler<M, E>
    {
        self.sender.ask(msg).await
    }

    pub(crate) fn new(path: ActorPath, sender: handler::MailboxSender<A, E>) -> Self {
        let handler = handler::HandlerRef::new(sender);
        ActorRef {
            path,
            sender: handler
        }
    }
}

impl<A: Actor<E>, E: SystemEvent> std::fmt::Debug for ActorRef<A, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

#[derive(Error, Debug)]
pub enum ActorError {

    #[error("Actor exists")]
    Exists(ActorPath),

    #[error("Actor runtime error")]
    Runtime(String)
}
