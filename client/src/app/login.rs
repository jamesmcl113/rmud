use std::time::Duration;

use crate::{
    client::{RawTask, TaskSpawner},
    CrosstermTerminal,
};

use super::{connected::Connected, App, AppState};
use crossterm::event::{self, KeyEvent};
use ratatui::widgets::Block;
use ratatui::{prelude::*, widgets::Borders};

use model::{ChatMessage, Response, ServerResponse, UserAction};
use tui_textarea::{CursorMove, TextArea};

/// `App` state when user is logging into server.
pub struct Login<'a> {
    text_field: TextArea<'a>,
}

impl<'a> App<Login<'a>> {
    pub fn new(terminal: CrosstermTerminal) -> Self {
        let (spawner, rx) = TaskSpawner::new();
        let mut text_field = TextArea::default();
        text_field.set_block(Block::default().borders(Borders::ALL));
        App {
            state: Login { text_field },
            spawner,
            rx,
            terminal,
        }
    }
}

impl AppState for Login<'_> {
    fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .constraints([
                Constraint::Percentage(30),
                Constraint::Max(3),
                Constraint::Percentage(30),
            ])
            .horizontal_margin(2)
            .split(f.size());

        f.render_widget(self.text_field.widget(), chunks[1]);
    }

    fn input(&mut self, key: KeyEvent, spawner: &mut TaskSpawner) -> bool {
        match key.code {
            event::KeyCode::Char(ch) => {
                self.text_field.insert_char(ch);
            }
            event::KeyCode::Esc => {
                return true;
            }
            event::KeyCode::Backspace => {
                self.text_field.delete_char();
            }
            event::KeyCode::Enter => {
                let msg = &self.text_field.lines()[0];
                if !msg.is_empty() {
                    spawner.spawn_task(RawTask {
                        req: UserAction::Chat(ChatMessage::Username(msg.to_string())),
                    });
                    self.text_field.move_cursor(CursorMove::End);
                    self.text_field.delete_line_by_head();
                }
            }
            _ => {}
        }

        false
    }
}

impl<'a> From<App<Login<'a>>> for App<Connected> {
    fn from(mut value: App<Login>) -> Self {
        let mut got_username = None;
        loop {
            // process input
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => {
                    if let event::Event::Key(key) = event::read().unwrap() {
                        if value.state.input(key, &mut value.spawner) {
                            // user has quit
                            break;
                        }
                    }
                }
                _ => {}
            }

            if let Ok(res) = value.rx.try_recv() {
                if let Response::Server(ServerResponse::JoinedServer { username }) = res {
                    got_username = Some(username);
                    break;
                }
            }

            // render
            value.terminal.draw(|f| value.state.render(f)).unwrap();
        }

        App {
            state: Connected {
                username: got_username.unwrap(),
            },
            spawner: value.spawner,
            rx: value.rx,
            terminal: value.terminal,
        }
    }
}
