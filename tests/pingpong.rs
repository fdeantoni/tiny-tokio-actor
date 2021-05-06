use async_trait::async_trait;
use uuid::Uuid;

use tiny_tokio_actor::{Actor, ActorContext, ActorSystem, EventBus, Handler, Message, SystemEvent};

#[derive(Clone)]
struct PingActor {
    counter: usize
}

impl Actor for PingActor {}

#[derive(Clone)]
struct PongActor;

impl Actor for PongActor {}

#[derive(Clone, Debug)]
enum PingPongMessage {
    Start(StartMessage),
    Ping(usize),
    Pong(usize),
    Done(usize)
}

#[derive(Clone, Debug)]
struct StartMessage {
    destination: Uuid,
    limit: usize
}

impl Message for PingPongMessage {
    type Response = PingPongMessage;
}

#[derive(Clone, Debug)]
struct EventMessage(String);

impl SystemEvent for EventMessage {}

#[async_trait]
impl Handler<PingPongMessage, EventMessage> for PingActor {
    async fn handle(&mut self, msg: PingPongMessage, ctx: &mut ActorContext<EventMessage>) -> PingPongMessage {
        if let PingPongMessage::Start(message) = msg {
            let limit = message.limit;
            if let Some(mut destination) = ctx.system.get_actor::<PongActor>(message.destination).await {
                while self.counter < limit {
                    self.counter += 1;
                    let response = PingPongMessage::Ping(self.counter);
                    let result = destination.ask(response).await.unwrap();
                    log::debug!("Got back {:?}", result);
                }
            }
        }

        PingPongMessage::Done(self.counter)
    }
}

#[async_trait]
impl Handler<PingPongMessage, EventMessage> for PongActor {
    async fn handle(&mut self, msg: PingPongMessage, _ctx: &mut ActorContext<EventMessage>) -> PingPongMessage {
        if let PingPongMessage::Ping(counter) = msg {
            PingPongMessage::Pong(counter + 1)
        } else {
            PingPongMessage::Done(0)
        }
    }
}

#[tokio::test]
async fn test_ping_pong() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "trace");
    }
    let _ = env_logger::builder().is_test(true).try_init();

    let bus = EventBus::<EventMessage>::new(1000);
    let system = ActorSystem::new("test", bus);

    let ping = PingActor { counter: 0 };
    let mut ping_ref = system.create_actor(ping).await;
    let pong = PongActor {};
    let pong_ref = system.create_actor(pong).await;

    let start = StartMessage {
        destination: *pong_ref.get_id(),
        limit: 5
    };

    let mut events = system.events();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => println!("Received event! {:?}", event),
                Err(err) => println!("Error receivng event!!! {:?}", err)
            }
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let result = ping_ref.ask(PingPongMessage::Start(start)).await.unwrap();
    println!("Final result: {:?}", result)
}