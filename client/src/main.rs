mod client;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};
use std::io::Stdout;
use std::time::Duration;
use tui_textarea::CursorMove;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use client::{Task, TaskSpawner};

use chrono::{DateTime, Local};

type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

struct ServerMessage {
    ty: MessageType,
    timestamp: DateTime<Local>,
}

enum MessageType {
    Chat(String),
    Server(String),
}

struct State<'a> {
    textarea: tui_textarea::TextArea<'a>,
    messages: Vec<ServerMessage>,
}

fn send_request(msg: &str, spawner: &TaskSpawner) -> Result<(), String> {
    if let Some(command) = msg.strip_prefix("/") {
        spawner.spawn_task(Task::send_command(command)?);
    } else {
        spawner.spawn_task(Task::send_chat(msg));
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = init_terminal()?;

    let mut textarea = tui_textarea::TextArea::default();
    textarea.set_placeholder_text("Enter some text.");
    textarea.set_block(Block::default().borders(Borders::ALL));
    textarea.set_cursor_line_style(Style::default());

    let mut state = State {
        textarea,
        messages: vec![],
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
                                match send_request(msg, &spawner) {
                                    Err(e) => {
                                        state.messages.push(ServerMessage {
                                            ty: MessageType::Server(format!("{e}")),
                                            timestamp: Local::now(),
                                        });
                                    }
                                    _ => {}
                                }
                                // clear line
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
            Ok(res) => match res {
                model::Response::Chat(msg) => {
                    let message = ServerMessage {
                        ty: MessageType::Chat(msg.msg),
                        timestamp: Local::now(),
                    };
                    state.messages.push(message);
                }
                model::Response::Game(_) => todo!(),
                model::Response::Server(msg) => {
                    let message = ServerMessage {
                        ty: MessageType::Server(msg),
                        timestamp: Local::now(),
                    };
                    state.messages.push(message);
                }
            },
            Err(_) => {}
        }
    }

    reset_terminal(&mut terminal)
}

fn render_message<'a>(msg: &'a ServerMessage) -> Vec<Line<'a>> {
    match &msg.ty {
        MessageType::Chat(contents) => vec![Line::from(vec![
            Span::styled(
                format!("[{}] ", msg.timestamp.format("%H:%M")),
                Style::new().yellow().bold(),
            ),
            Span::from(contents),
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
    }
}

fn ui(f: &mut Frame, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Percentage(80), Constraint::Max(3)])
        .split(f.size());

    let max_messages = chunks[0].height - 2;

    let messages = state
        .messages
        .iter()
        .map(|msg| render_message(msg))
        .flatten()
        .collect::<Vec<_>>();

    let n_messages = messages.len() as u16;

    let offset = u16::max(n_messages.saturating_sub(max_messages), 0);

    let para = Paragraph::new(messages)
        .scroll((offset, 0))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(para, chunks[0]);
    f.render_widget(state.textarea.widget(), chunks[1]);
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
