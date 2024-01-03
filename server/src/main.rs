mod request;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use model::Response;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::bytes::Bytes;
use tokio_util::codec::{BytesCodec, Framed};

use futures::SinkExt;

pub struct UserSession {
    name: String,
    room: usize,
    send: mpsc::UnboundedSender<model::Response>,
}

struct User {
    rx: mpsc::UnboundedReceiver<model::Response>,
    bytes: Framed<TcpStream, BytesCodec>,
}

impl User {
    pub async fn new(
        state: Arc<Mutex<Shared>>,
        lines: Framed<TcpStream, BytesCodec>,
        name: &str,
    ) -> std::io::Result<User> {
        let addr = lines.get_ref().peer_addr()?;

        let (tx, rx) = mpsc::unbounded_channel();

        let mut state = state.lock().await;
        let session = UserSession {
            name: name.to_string(),
            room: 0,
            send: tx,
        };
        state.peers.insert(addr, session);

        Ok(User { bytes: lines, rx })
    }
}

// maybe use a hashmap<addr, UserData> to hold name, current room etc
// would make adding and removing users easier
pub struct Shared {
    peers: HashMap<SocketAddr, UserSession>,
    rooms: Vec<String>,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            peers: HashMap::new(),
            rooms: vec![String::from("main"), String::from("general")],
        }
    }

    fn move_user_to_room(&mut self, user: &SocketAddr, room_name: &str) -> Result<String, String> {
        let session = self.peers.get_mut(user).ok_or("No session for address")?;
        let (room_idx, _) = self
            .rooms
            .iter()
            .enumerate()
            .find(|&(_, r)| r == room_name)
            .ok_or(format!("Room '{room_name}' does not exist"))?;

        session.room = room_idx;

        Ok(String::from("main"))
    }

    async fn private_message(&self, msg: &str, from: &SocketAddr, to: &str) -> Result<(), String> {
        let (dest_addr, session) = self
            .peers
            .iter()
            .find(|&(_, session)| session.name == to)
            .ok_or(format!("Cant find socket for user {to}"))?;

        if from == dest_addr {
            return Ok(());
        }

        let from_name = &self.peers.get(from).ok_or("Couldn't find from user.")?.name;

        session
            .send
            .send(model::Response::Chat(model::ChatMessage::Private(
                model::Message {
                    payload: msg.to_string(),
                    from: from_name.to_owned(),
                },
            )))
            .map_err(|e| format!("Error sending pm: {e:?}"))
    }

    async fn broadcast(&mut self, sender: &SocketAddr, message: &str, from: Option<&str>) {
        let res = match from {
            Some(sender) => model::Response::Chat(model::ChatMessage::Public(model::Message {
                payload: message.to_string(),
                from: sender.to_string(),
            })),
            None => model::Response::Server(model::ServerResponse::General {
                msg: message.to_string(),
            }),
        };
        let cur_room = self.peers.get(sender).unwrap().room;
        let users_in_room = self.peers.values().filter(|&s| s.room == cur_room);
        for peer in users_in_room {
            peer.send.send(res.clone()).unwrap();
        }
    }

    fn get_users(&self) -> impl Iterator<Item = &String> {
        self.peers.values().map(|p| &p.name)
    }

    pub fn remove_user(&mut self, addr: &SocketAddr) -> Option<UserSession> {
        self.peers.remove(addr)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let state = Arc::new(Mutex::new(Shared::new()));

    loop {
        let (socket, addr) = listener.accept().await?;
        let state = Arc::clone(&state);

        tokio::spawn(async move {
            if let Err(e) = process(socket, state, addr).await {
                eprintln!("Failed to process user: error = {:?}", e)
            }
        });
    }
}

async fn send_response(stream: &mut Framed<TcpStream, BytesCodec>, res: Response) {
    let res_bytes: Vec<u8> = res.into();
    stream.send(Bytes::from(res_bytes)).await.unwrap();
}

async fn process(
    stream: TcpStream,
    state: Arc<Mutex<Shared>>,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Framed::new(stream, BytesCodec::new());

    send_response(&mut bytes, Response::server_msg("Please enter a username:")).await;

    let name: String = match bytes.next().await {
        Some(Ok(line)) => String::from_utf8(line.to_vec()).unwrap(),
        _ => {
            eprintln!("Failed to get username. Client disconnected.");
            return Ok(());
        }
    };

    let mut user = User::new(state.clone(), bytes, &name).await?;

    {
        let mut state = state.lock().await;
        let room_name = state.move_user_to_room(&addr, "main")?;

        // do some validation here...
        let b_msg = format!("{} has joined the chat.", name);
        state.broadcast(&addr, &b_msg, None).await;
        send_response(
            &mut user.bytes,
            Response::Server(model::ServerResponse::JoinedServer {
                room_name,
                username: name.clone(),
            }),
        )
        .await;
    }

    loop {
        tokio::select! {
            // client received a message
            Some(msg) = user.rx.recv() => {
                send_response(&mut user.bytes, msg).await;
            }
            // client has sent a message
            result = user.bytes.next() => match result {
                Some(Ok(msg)) => {
                    request::handle_request(msg, state.clone(), &mut user.bytes, &addr, &name).await;
                }
                Some(Err(e)) => {
                    eprintln!(
                        "an error occurred while processing messages for {}. err = {e:?}",
                        name
                    )
                }
                None => break,
            }
        }
    }

    // client disconnected
    {
        let mut state = state.lock().await;
        let msg = format!("{name} has left the chat.");
        state.broadcast(&addr, &msg, None).await;
        state.remove_user(&addr);
    }

    Ok(())
}
