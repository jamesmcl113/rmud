use serde::{Deserialize, Serialize};

impl Into<Vec<u8>> for UserAction {
    fn into(self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserAction {
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

    pub fn public_msg(msg: &str, room_name: &str, from: &str) -> Response {
        Response::Chat(ChatMessage::Public {
            room_name: room_name.to_string(),
            from: from.to_string(),
            msg: msg.to_string(),
        })
    }

    pub fn private_msg(msg: &str, from: &str) -> Response {
        Response::Chat(ChatMessage::Private {
            from: from.to_string(),
            msg: msg.to_string(),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ChatMessage {
    Private {
        from: String,
        msg: String,
    },
    Public {
        room_name: String,
        from: String,
        msg: String,
    },
    Username(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameUpdate {
    pub msg: String,
}
