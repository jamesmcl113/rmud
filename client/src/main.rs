mod app;
mod client;
mod ui;

use app::{connected::Connected, App};
use model::{ChatMessage, UserAction};
use ratatui::prelude::*;
use ratatui::Terminal;
use std::collections::HashMap;
use std::io::Stdout;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use client::{RawTask, TaskSpawner};

use chrono::{DateTime, Local};

pub type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct ServerMessage {
    ty: MessageType,
    timestamp: DateTime<Local>,
}

pub enum MessageType {
    Public { msg: String, from: String },
    Private { msg: String, from: String },
    Server(String),
}

pub struct UserData {
    username: String,
}

pub struct State<'a> {
    textarea: tui_textarea::TextArea<'a>,
    room_messages: HashMap<String, Vec<ServerMessage>>,
    debug_messages: Vec<String>,
    show_debug: bool,
    user_data: Option<UserData>,
    current_tab: Option<String>,
}

impl State<'_> {
    pub fn handle_response(&mut self, res: model::Response) {
        let timestamp = Local::now();

        match res {
            model::Response::Chat(chat) => match chat {
                ChatMessage::Private { from, msg } => todo!(),
                ChatMessage::Public {
                    room_name,
                    from,
                    msg,
                } => {
                    let room_log = self
                        .room_messages
                        .get_mut(&room_name)
                        .expect("Got a message from an unknown room");

                    room_log.push(ServerMessage {
                        ty: MessageType::Public { msg, from },
                        timestamp,
                    });
                }
                ChatMessage::Username(_) => todo!(),
            },
            model::Response::Game(_) => todo!(),
            model::Response::Server(res) => self.handle_server_response(res),
        }
    }

    pub fn handle_server_response(&mut self, res: model::ServerResponse) {
        match res {
            model::ServerResponse::JoinedServer { username } => {
                self.debug_messages.push("Joined server".to_string());
                match self.user_data {
                    None => self.user_data = Some(UserData { username }),
                    Some(_) => {
                        panic!("Received extra JoinedServer response!")
                    }
                }
            }
            model::ServerResponse::JoinedRoom { room_name } => {
                self.debug_messages.push("Joined room".to_string());
                match self.room_messages.insert(room_name.clone(), vec![]) {
                    Some(_) => panic!("Received duplicate JoinedRoom res for {room_name}"),
                    None => self.current_tab = Some(room_name.clone()),
                }
            }
            model::ServerResponse::OtherUserJoined { name } => todo!(),
            model::ServerResponse::General { room_name, msg } => {
                let room_buffer = self.room_messages.get_mut(&room_name).unwrap();
                room_buffer.push(ServerMessage {
                    ty: MessageType::Server(msg),
                    timestamp: Local::now(),
                });
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = init_terminal()?;

    let app = App::new(terminal);

    // Start login process
    let mut app_connected = App::<Connected>::from(app);

    reset_terminal(&mut app_connected.terminal)
}

fn reset_terminal(terminal: &mut CrosstermTerminal) -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(terminal.show_cursor()?)
}

fn init_terminal() -> Result<CrosstermTerminal, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}
