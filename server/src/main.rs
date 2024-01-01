use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use model::Response;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::StreamExt;
use tokio_util::bytes::{Bytes, BytesMut};
use tokio_util::codec::{BytesCodec, Framed, LinesCodec};

use futures::SinkExt;

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
        state.peers.insert(addr, tx);
        state.names.insert(addr, name.to_string());

        Ok(User { bytes: lines, rx })
    }
}

struct Shared {
    peers: HashMap<SocketAddr, mpsc::UnboundedSender<model::Response>>,
    names: HashMap<SocketAddr, String>,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            peers: HashMap::new(),
            names: HashMap::new(),
        }
    }

    async fn broadcast(&mut self, sender: SocketAddr, message: &str) {
        let res = model::Response::Chat(model::ChatMessage {
            msg: message.to_string(),
        });
        for peer in self.peers.iter_mut() {
            let _ = peer.1.send(res.clone());
        }
    }

    fn get_users(&self) -> impl Iterator<Item = &String> {
        self.names.values()
    }

    pub fn remove_user(&mut self, addr: &SocketAddr) -> Option<()> {
        if self.peers.remove(addr).is_some() && self.names.remove(addr).is_some() {
            Some(())
        } else {
            None
        }
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

    let name_res: model::Request = match bytes.next().await {
        Some(Ok(line)) => bincode::deserialize(&line[..]).unwrap(),
        _ => {
            eprintln!("Failed to get username. Client disconnected.");
            return Ok(());
        }
    };

    let name = match name_res {
        model::Request::UserMessage(msg) => msg,
        _ => {
            eprintln!("Wrong format. Expected Respone::Game(_) with username.");
            return Ok(());
        }
    };

    let mut user = User::new(state.clone(), bytes, &name).await?;

    {
        let mut state = state.lock().await;

        // do some validation here...
        let b_msg = format!("{} has joined the chat.", name);
        state.broadcast(addr, &b_msg).await;
    }

    loop {
        tokio::select! {
            // user received a message
            Some(msg) = user.rx.recv() => {
                send_response(&mut user.bytes, msg).await;
            }
            // client has sent a message
            result = user.bytes.next() => match result {
                Some(Ok(msg)) => {
                    let req: model::Request = bincode::deserialize(&msg[..]).unwrap();
                    let mut state = state.lock().await;
                    match req {
                        model::Request::UserMessage(msg) => {
                            state.broadcast(addr, format!("{name}: {msg}").as_str()).await;
                        },
                        model::Request::UserAction(act) => {
                            match act {
                                model::UserActions::GetUsers => {
                                    let mut user_list = String::from("Users in room:\n");
                                    for user in state.get_users() {
                                        user_list.push_str(user);
                                        user_list.push('\n');
                                    }
                                    send_response(&mut user.bytes, Response::Server(user_list)).await;
                                },
                            }
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

    // user disconnected
    {
        let mut state = state.lock().await;
        state.remove_user(&addr);

        let msg = format!("{name} has left the chat.");
        state.broadcast(addr, &msg).await;
    }

    Ok(())
}
