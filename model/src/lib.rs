use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Request {
    UserMessage(String),
    UserAction(UserActions),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserActions {
    GetUsers,
}

impl Into<Vec<u8>> for Request {
    fn into(self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Response {
    Chat(ChatMessage),
    Game(GameUpdate),
    Server(String),
}

impl Into<Vec<u8>> for Response {
    fn into(self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
}

impl Response {
    pub fn server_msg(msg: &str) -> Response {
        Response::Server(msg.to_string())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub msg: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameUpdate {
    pub msg: String,
}
