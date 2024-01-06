mod client;

use model::{ChatMessage, Message, UserAction, UserActions};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};
use std::collections::HashMap;
use std::io::Stdout;
use std::time::Duration;
use tui_textarea::CursorMove;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use client::{RawTask, TaskSpawner};

use chrono::{DateTime, Local};

type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

struct ServerMessage {
    ty: MessageType,
    timestamp: DateTime<Local>,
}

enum MessageType {
    Public(model::Message),
    Private(model::Message),
    Server(String),
}

struct UserData {
    username: String,
}

struct State<'a> {
    textarea: tui_textarea::TextArea<'a>,
    room_messages: HashMap<String, Vec<ServerMessage>>,
    user_data: Option<UserData>,
}

impl State<'_> {
    pub fn handle_response(&mut self, res: model::Response) {
        let timestamp = Local::now();

        match res {
            model::Response::Chat(_) => todo!(),
            model::Response::Game(_) => todo!(),
            model::Response::Server(res) => self.handle_server_response(res),
        }
    }

    pub fn handle_server_response(&mut self, res: model::ServerResponse) {
        match res {
            model::ServerResponse::JoinedServer { username } => match self.user_data {
                None => self.user_data = Some(UserData { username }),
                Some(_) => {
                    panic!("Received extra JoinedServer response!")
                }
            },
            model::ServerResponse::JoinedRoom { room_name } => {
                match self.room_messages.insert(room_name, vec![]) {
                    Some(_) => panic!("Received duplicate JoinedRoom res for {room_name}"),
                    None => {}
                }
            }
            model::ServerResponse::OtherUserJoined { name } => todo!(),
            model::ServerResponse::General { room_name, msg } => todo!(),
        }
    }
}

fn send_request(msg: &str, name: &str, spawner: &TaskSpawner) {
    spawner.spawn_task(RawTask {
        req: UserAction {
            room_name: "main".to_string(),
            action: UserActions::Chat(ChatMessage::Public(Message {
                payload: msg.to_string(),
                from: name.to_string(),
            })),
        },
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = init_terminal()?;

    let mut textarea = tui_textarea::TextArea::default();
    textarea.set_placeholder_text("Enter some text.");
    textarea.set_block(Block::default().borders(Borders::ALL));
    textarea.set_cursor_line_style(Style::default());

    let mut state = State {
        textarea,
        room_messages: HashMap::new(),
        user_data: None,
    };

    let (spawner, mut rx) = TaskSpawner::new();

    loop {
        terminal.draw(|f| ui(f, &state))?;

        match event::poll(Duration::from_millis(100)) {
            Ok(true) => {
                if let event::Event::Key(key) = event::read()? {
                    match key.code {
                        event::KeyCode::Char(ch) => {
                            state.textarea.insert_char(ch);
                        }
                        event::KeyCode::Backspace => {
                            state.textarea.delete_char();
                        }
                        event::KeyCode::Esc => {
                            break;
                        }
                        event::KeyCode::Enter => {
                            let msg = &state.textarea.lines()[0];
                            if !msg.is_empty() {
                                let name = &state.user_data.unwrap().username;
                                send_request(msg, name, &spawner);
                                state.textarea.move_cursor(CursorMove::End);
                                state.textarea.delete_line_by_head();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        match rx.try_recv() {
            Ok(res) => state.handle_response(res),
            Err(_) => {}
        }
    }

    reset_terminal(&mut terminal)
}

fn render_message<'a>(msg: &'a ServerMessage) -> Vec<Line<'a>> {
    match &msg.ty {
        MessageType::Public(model::Message { payload, from }) => vec![Line::from(vec![
            Span::styled(
                format!("[{}] ", msg.timestamp.format("%H:%M")),
                Style::new().yellow().bold(),
            ),
            Span::styled(format!("{from}: "), Style::new().bold()),
            Span::from(payload),
        ])],
        MessageType::Server(contents) => {
            let mut lines = vec![];
            for (i, line) in contents.split("\n").enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled("[SERVER] ", Style::new().red().bold()),
                        Span::from(line),
                    ]))
                } else {
                    lines.push(Line::from(format!("\t{line}")))
                }
            }

            lines
        }
        MessageType::Private(model::Message { from, payload }) => {
            vec![Line::from(vec![
                Span::styled(
                    format!("[{}] ", msg.timestamp.format("%H:%M")),
                    Style::new().green().bold(),
                ),
                Span::styled(format!("{from}: "), Style::new().green().bold()),
                Span::styled(payload, Style::new().green().bold()),
            ])]
        }
    }
}

fn ui(f: &mut Frame, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Max(1),
            Constraint::Percentage(80),
            Constraint::Max(3),
        ])
        .split(f.size());

    let max_messages = chunks[1].height - 2;

    let messages = state
        .room_messages
        .iter()
        .map(|msg| render_message(msg))
        .flatten()
        .collect::<Vec<_>>();

    let n_messages = messages.len() as u16;

    let offset = u16::max(n_messages.saturating_sub(max_messages), 0);

    let para = Paragraph::new(messages)
        .scroll((offset, 0))
        .block(Block::default().borders(Borders::ALL));

    let status_line = match &state.user_data {
        Some(UserData {
            username,
            room_name,
        }) => Paragraph::new(Line::from(vec![
            Span::styled(username, Style::new().bold()),
            Span::from(" in #"),
            Span::styled(room_name, Style::new().yellow()),
        ])),
        None => Paragraph::new("..."),
    };

    f.render_widget(status_line, chunks[0]);
    f.render_widget(para, chunks[1]);
    f.render_widget(state.textarea.widget(), chunks[2]);
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
