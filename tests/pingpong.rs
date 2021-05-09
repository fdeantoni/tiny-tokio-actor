use tiny_tokio_actor::*;

#[derive(Clone, Debug)]
struct EventMessage(String);

impl SystemEvent for EventMessage {}

#[derive(Clone)]
struct PingActor {
    counter: i8
}

impl Actor<EventMessage> for PingActor {}

#[derive(Clone)]
struct PongActor;

impl Actor<EventMessage> for PongActor {}

#[derive(Clone, Debug)]
enum PingMessage {
    Start(StartMessage),
    Ping(i8),
}

#[derive(Clone, Debug)]
struct StartMessage {
    destination: ActorPath,
    limit: i8
}

impl Message for PingMessage {
    type Response = PongMessage;
}

#[derive(Clone, Debug)]
struct PongMessage(i8);

impl Message for PongMessage {
    type Response = ();
}

#[async_trait]
impl Handler<PingMessage, EventMessage> for PingActor {
    async fn handle(&mut self, msg: PingMessage, ctx: &mut ActorContext<EventMessage>) -> PongMessage {
        if let PingMessage::Start(message) = msg {
            let limit = message.limit;
            if let Some(mut destination) = ctx.system.get_actor::<PongActor>(&message.destination).await {
                while self.counter > -1 && self.counter < limit {
                    let ping = PingMessage::Ping(self.counter);
                    let result = destination.ask(ping).await.unwrap();
                    self.counter = result.0;
                    ctx.system.publish(EventMessage(format!("Counter is now {}", self.counter)));
                }
            }
        }

        PongMessage(self.counter)
    }
}

#[async_trait]
impl Handler<PingMessage, EventMessage> for PongActor {
    async fn handle(&mut self, msg: PingMessage, _ctx: &mut ActorContext<EventMessage>) -> PongMessage {
        if let PingMessage::Ping(counter) = msg {
            PongMessage(counter + 1)
        } else {
            PongMessage(-1)
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
    let mut ping_ref = system.create_actor("ping", ping).await.unwrap();
    let pong = PongActor {};
    let pong_ref = system.create_actor("pong", pong).await.unwrap();

    let start = StartMessage {
        destination: pong_ref.get_path().clone(),
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

    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;

    let result = ping_ref.ask(PingMessage::Start(start)).await.unwrap();
    println!("Final result: {:?}", &result);
    assert_eq!(result.0, 5);
}