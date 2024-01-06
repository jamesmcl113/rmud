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
    username: &str,
    current_room: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let msg = String::from_utf8(data.to_vec())?;
    let mut state = state.lock().await;

    if let Some(command_msg) = msg.strip_prefix("/") {
        // command
        let tokens = command_msg.split_whitespace().collect::<Vec<_>>();
        let (cmd, args) = tokens.split_first().ok_or("Expected a command.")?;

        match *cmd {
            "who" => {
                get_users(&state, stream, current_room).await;
            }
            "rooms" | "rs" => {
                get_rooms(&state, stream, current_room).await;
            }
            "pm" => {
                let to = args.get(0).ok_or("Error with pm: Expected a recepient.")?;
                let full_message = &args[1..].join(" ");

                match state.private_message(&full_message, addr, to).await {
                    Ok(_) => {}
                    Err(_) => {
                        let res = Response::server_msg(
                            &format!("Couldn't send PM to {to}"),
                            current_room,
                        );
                        send_response(stream, res).await;
                    }
                }
            }
            _ => {}
        }
    } else {
        // public message
        state
            .broadcast(addr, &msg, Some(username), current_room)
            .await;
    }

    Ok(())
}

async fn get_rooms(state: &Shared, stream: &mut Stream, current_room: &str) {
    let mut room_list = String::from("Joinable rooms:\n");
    for (room_name, _) in &state.rooms {
        room_list.push_str(room_name);
        room_list.push('\n');
    }
    send_response(stream, Response::server_msg(&room_list, current_room)).await;
}

async fn get_users(state: &Shared, stream: &mut Stream, current_room: &str) {
    let mut user_list = String::from("Users in room:\n");
    for user in state.get_users() {
        user_list.push_str(user);
        user_list.push('\n');
    }
    send_response(stream, Response::server_msg(&user_list, current_room)).await;
}

pub async fn send_response(stream: &mut Stream, res: Response) {
    let res_bytes: Vec<u8> = res.into();
    stream.send(Bytes::from(res_bytes)).await.unwrap();
}
