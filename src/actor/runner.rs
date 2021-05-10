use crate::system::{ActorSystem, SystemEvent};

use super::{Actor, ActorContext, ActorRef, ActorPath, handler::{ActorMailbox, MailboxReceiver}};

pub(crate) struct ActorRunner<E: SystemEvent, A: Actor<E>> {
    path: ActorPath,
    actor: A,
    receiver: MailboxReceiver<E, A>,
}

impl<E: SystemEvent, A: Actor<E>> ActorRunner<E, A> {

    pub fn create(path: ActorPath, actor: A) -> (Self, ActorRef<E, A>) {
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

        self.actor.pre_start(&mut ctx).await;

        while let Some(mut msg) = self.receiver.recv().await {
            msg.handle(&mut self.actor, &mut ctx).await;
        }

        self.actor.post_stop(&mut ctx).await;

        log::debug!("Actor '{}' stopped.", &self.path);
    }
}