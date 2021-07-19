use std::net::SocketAddr;
use std::str::FromStr;

use futures::StreamExt;
use tokio::task;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

use uuid::Uuid;
use warp::*;
use warp::ws::WebSocket;

use tiny_tokio_actor::*;

#[derive(Clone, Debug)]
struct ServerEvent(String);

// Mark the struct as a system event message.
impl SystemEvent for ServerEvent {}

#[tokio::main]
async fn main() {
    let path = std::path::Path::new(".env");
    dotenv::from_path(path).ok();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,tiny_tokio_actor=debug,websocket=debug");
    }
    env_logger::init();

    let addr = std::env::var("HOST_PORT")
        .ok()
        .and_then(|string| SocketAddr::from_str(&string).ok())
        .unwrap_or_else(|| SocketAddr::from_str("127.0.0.1:9000").unwrap());

    // Create the event bus and actor system
    let bus = EventBus::<ServerEvent>::new(1000);
    let system = ActorSystem::new("test", bus);

    // Create the warp WebSocket route
    let ws = warp::path!("echo")
        .and(warp::any().map(move || system.clone()))
        .and(warp::addr::remote())
        .and(warp::ws())
        .map(|system: ActorSystem<ServerEvent>, remote: Option<SocketAddr>, ws: warp::ws::Ws| {
            ws.on_upgrade(move |websocket| start_echo(system, remote, websocket) )
        });

    // Create the warp routes (websocket only in this case, with warp logging added)
    let routes = ws.with(warp::log("echo-server"));

    // Start the server
    warp::serve(routes).run(addr).await;
}

// Starts a new echo actor on our actor system
async fn start_echo(system: ActorSystem<ServerEvent>, remote: Option<SocketAddr>, websocket: WebSocket) {

    // Split out the websocket into incoming and outgoing
    let (ws_out, mut ws_in) = websocket.split();

    // Create an unbounded channel where the actor can send its responses to ws_out
    let (sender, receiver) = mpsc::unbounded_channel();
    let receiver = UnboundedReceiverStream::new(receiver);
    task::spawn(receiver.map(Ok).forward(ws_out));

    // Create a new echo actor with the newly created sender
    let actor = EchoActor::new(sender);
    // Use the websocket client address to generate a unique actor name
    let addr = remote
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string() );
    let actor_name = format!("echo-actor-{}", &addr);
    // Launch the actor on our actor system
    let mut actor_ref = system.create_actor(&actor_name, actor).await.unwrap();

    // Loop over all websocket messages received over ws_in
    while let Some(result) = ws_in.next().await {
        // If no error, we tell the websocket message to the echo actor, otherwise break the loop
        match result {
            Ok(msg) => actor_ref.tell(EchoRequest(msg)).unwrap(),
            Err(error) => {
                ::log::error!("error processing ws message from {}: {:?}", &addr, error);
                break;
            }
        };
    }

    // The loop has been broken, kill the echo actor
    system.stop_actor(actor_ref.path()).await;
}

#[derive(Clone)]
struct EchoActor {
    sender: mpsc::UnboundedSender<warp::ws::Message>
}

impl EchoActor {
    pub fn new(sender: mpsc::UnboundedSender<warp::ws::Message>) -> Self {
        EchoActor {
            sender
        }
    }
}

impl Actor<ServerEvent> for EchoActor {}

#[derive(Clone, Debug)]
struct EchoRequest(warp::ws::Message);

impl Message for EchoRequest {
    type Response = ();
}

#[async_trait]
impl Handler<ServerEvent, EchoRequest> for EchoActor {
    async fn handle(&mut self, msg: EchoRequest, ctx: &mut ActorContext<ServerEvent>) {
        ::log::debug!("actor {} on system {} received message! {:?}", &ctx.path, ctx.system.name(), &msg);
        self.sender.send(msg.0).unwrap()
    }
}