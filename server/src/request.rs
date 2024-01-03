use crate::Shared;
use futures::SinkExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_util::{
    bytes::{Bytes, BytesMut},
    codec::{BytesCodec, Framed},
};

use std::error::Error;

use model::{Response, ServerResponse};

type Stream = Framed<TcpStream, BytesCodec>;

pub async fn handle_request(
    data: BytesMut,
    state: Arc<Mutex<Shared>>,
    stream: &mut Stream,
    addr: &SocketAddr,
    name: &str,
) -> Result<(), Box<dyn Error>> {
    let msg = String::from_utf8(data.to_vec())?;
    let mut state = state.lock().await;

    if let Some(command_msg) = msg.strip_prefix("/") {
        // command
        let tokens = command_msg.split_whitespace().collect::<Vec<_>>();
        let (cmd, args) = tokens.split_first().ok_or("Expected a command.")?;

        match *cmd {
            "who" => {
                get_users(&state, stream).await;
            }
            "rooms" | "rs" => {
                get_rooms(&state, stream).await;
            }
            "pm" => {
                let to = args.get(0).ok_or("Error with pm: Expected a recepient.")?;
                let full_message = &args[1..].join(" ");

                match state.private_message(&full_message, addr, to).await {
                    Ok(_) => {}
                    Err(_) => {
                        let res = Response::Server(ServerResponse::General {
                            msg: format!("Couldn't send message to {to}"),
                        });
                        send_response(stream, res).await;
                    }
                }
            }
            "mv" => {
                let room_name = args.get(0).ok_or("Error with mv: Expected a room name.")?;
                let res = match state.move_user_to_room(addr, &room_name) {
                    Ok(_) => Response::Server(ServerResponse::JoinedRoom {
                        room_name: room_name.to_string(),
                    }),
                    Err(e) => Response::Server(ServerResponse::General {
                        msg: format!("Couldn't join {room_name}. err = {e}"),
                    }),
                };

                send_response(stream, res).await;
            }
            _ => {}
        }
    } else {
        // public message
        state.broadcast(addr, &msg, Some(name)).await;
    }

    Ok(())
}

async fn get_rooms(state: &Shared, stream: &mut Framed<TcpStream, BytesCodec>) {
    let mut room_list = String::from("Joinable rooms:\n");
    for room_name in &state.rooms {
        room_list.push_str(room_name);
        room_list.push('\n');
    }
    send_response(
        stream,
        Response::Server(model::ServerResponse::General { msg: room_list }),
    )
    .await;
}

async fn get_users(state: &Shared, stream: &mut Stream) {
    let mut user_list = String::from("Users in room:\n");
    for user in state.get_users() {
        user_list.push_str(user);
        user_list.push('\n');
    }
    send_response(
        stream,
        Response::Server(model::ServerResponse::General { msg: user_list }),
    )
    .await;
}

async fn send_response(stream: &mut Framed<TcpStream, BytesCodec>, res: Response) {
    let res_bytes: Vec<u8> = res.into();
    stream.send(Bytes::from(res_bytes)).await.unwrap();
}
