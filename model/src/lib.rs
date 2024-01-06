use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserAction {
    pub room_name: String,
    pub action: UserActions,
}

impl Into<Vec<u8>> for UserAction {
    fn into(self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserActions {
    Chat(ChatMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Response {
    Chat(ChatMessage),
    Game(GameUpdate),
    Server(ServerResponse),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerResponse {
    JoinedServer { username: String },
    JoinedRoom { room_name: String },
    OtherUserJoined { name: String },
    General { room_name: String, msg: String },
}

impl Into<Vec<u8>> for Response {
    fn into(self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
}

impl Response {
    pub fn server_msg(msg: &str, room_name: &str) -> Response {
        Response::Server(ServerResponse::General {
            msg: msg.to_string(),
            room_name: room_name.to_string(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChatMessage {
    Private(Message),
    Public(Message),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub payload: String,
    pub from: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameUpdate {
    pub msg: String,
}
