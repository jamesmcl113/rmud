mod request;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use model::{Response, UserAction};
use request::send_response;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Framed};

pub struct UserSession {
    name: String,
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
    ) -> std::io::Result<(User, String)> {
        let addr = lines.get_ref().peer_addr()?;

        let (tx, rx) = mpsc::unbounded_channel();

        let mut state = state.lock().await;
        let name = state.gen_username();
        let session = UserSession {
            name: name.clone(),
            send: tx,
        };
        state.add_user(addr, session);

        Ok((User { bytes: lines, rx }, name))
    }
}

// maybe use a hashmap<addr, UserData> to hold name, current room etc
// would make adding and removing users easier
pub struct Shared {
    peers: HashMap<SocketAddr, UserSession>,
    rooms: HashMap<String, Vec<SocketAddr>>,
    user_count: usize,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            peers: HashMap::new(),
            rooms: HashMap::from([(String::from("main"), vec![])]),
            user_count: 0,
        }
    }

    fn add_user(&mut self, addr: SocketAddr, session: UserSession) {
        self.peers.insert(addr, session);
        self.user_count += 1;
    }

    fn gen_username(&self) -> String {
        format!("User-{}", self.user_count)
    }

    fn add_user_to_room(&mut self, user: &SocketAddr, room_name: &str) -> Result<(), String> {
        let session = self.peers.get_mut(user).ok_or("No session for address")?;

        let room = self
            .rooms
            .get_mut(room_name)
            .ok_or(format!("Room {room_name} does not exist."))?;

        room.push(*user);

        session
            .send
            .send(Response::Server(model::ServerResponse::JoinedRoom {
                room_name: room_name.to_string(),
            }))
            .unwrap();

        Ok(())
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

    async fn broadcast(
        &mut self,
        sender: &SocketAddr,
        message: &str,
        from: Option<&str>,
        room_name: &str,
    ) {
        let res = match from {
            Some(sender) => model::Response::Chat(model::ChatMessage::Public(model::Message {
                payload: message.to_string(),
                from: sender.to_string(),
            })),
            None => model::Response::Server(model::ServerResponse::General {
                msg: message.to_string(),
                room_name: room_name.to_string(),
            }),
        };
        let room = self.rooms.get(room_name).unwrap();
        let users_in_room = room.iter().filter_map(|user| self.peers.get(user));
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

async fn process(
    stream: TcpStream,
    state: Arc<Mutex<Shared>>,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Framed::new(stream, BytesCodec::new());
    let (mut user, name) = User::new(state.clone(), bytes).await?;

    {
        let mut state = state.lock().await;
        state.add_user_to_room(&addr, "main")?;

        // do some validation here...
        let b_msg = format!("{} has joined the chat.", name);
        state.broadcast(&addr, &b_msg, None, "main").await;
        send_response(
            &mut user.bytes,
            Response::Server(model::ServerResponse::JoinedServer {
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
                    let req: UserAction = bincode::deserialize(&msg[..]).unwrap();
                    match request::handle_request(msg, state.clone(), &mut user.bytes, &addr, &name, &req.room_name).await {
                        Ok(_) => {},
                        Err(e) => {
                            send_response(&mut user.bytes, Response::server_msg(&format!("ERROR: {e}"), &req.room_name)).await;
                        },
                    }
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
        state.broadcast(&addr, &msg, None, "main").await;
        state.remove_user(&addr);
    }

    Ok(())
}
