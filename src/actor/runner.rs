use uuid::Uuid;

use crate::system::{ActorSystem, SystemEvent};

use super::{Actor, ActorContext, ActorRef, handler::{ActorMailbox, MailboxReceiver}};

pub struct ActorRunner<A: Actor, E: SystemEvent> {
    id: Uuid,
    actor: A,
    receiver: MailboxReceiver<A, E>,
}

impl<A: Actor, E: SystemEvent> ActorRunner<A, E> {

    pub fn create(actor: A) -> (Self, ActorRef<A, E>) {
        let (sender, receiver) = ActorMailbox::create();
        let id = Uuid::new_v4();
        let actor_ref = ActorRef::new(id, sender);
        let runner = ActorRunner {
            id,
            actor,
            receiver,
        };
        (runner, actor_ref)
    }

    pub async fn start(&mut self, system: ActorSystem<E>) {

        log::debug!("Starting actor '{}'...", &self.id);

        let mut ctx = ActorContext {
            id: self.id,
            system,
        };

        while let Some(mut msg) = self.receiver.recv().await {
            msg.handle(&mut self.actor, &mut ctx).await;
        }

        log::debug!("Actor '{}' stopped.", &self.id);
    }
}