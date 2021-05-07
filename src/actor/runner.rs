use crate::system::{ActorSystem, SystemEvent};

use super::{Actor, ActorContext, ActorRef, ActorPath, handler::{ActorMailbox, MailboxReceiver}};

pub struct ActorRunner<A: Actor, E: SystemEvent> {
    path: ActorPath,
    actor: A,
    receiver: MailboxReceiver<A, E>,
}

impl<A: Actor, E: SystemEvent> ActorRunner<A, E> {

    pub fn create(path: ActorPath, actor: A) -> (Self, ActorRef<A, E>) {
        let (sender, receiver) = ActorMailbox::create();
        let actor_ref = ActorRef::new(path.clone(), sender);
        let runner = ActorRunner {
            path,
            actor,
            receiver,
        };
        (runner, actor_ref)
    }

    pub async fn start(&mut self, system: ActorSystem<E>) {

        log::debug!("Starting actor '{}'...", &self.path);

        let mut ctx = ActorContext {
            path: self.path.clone(),
            system,
        };

        while let Some(mut msg) = self.receiver.recv().await {
            msg.handle(&mut self.actor, &mut ctx).await;
        }

        log::debug!("Actor '{}' stopped.", &self.path);
    }
}