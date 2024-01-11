pub mod connected;
pub mod login;

use crossterm::event::KeyEvent;
use model::Response;
use ratatui::prelude::*;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{CrosstermTerminal, TaskSpawner};

pub struct App<S: AppState> {
    pub state: S,
    spawner: TaskSpawner,
    rx: UnboundedReceiver<Response>,
    pub terminal: CrosstermTerminal,
}

pub trait AppState {
    fn render(&self, f: &mut Frame);
    fn input(&mut self, key: KeyEvent, spawner: &mut TaskSpawner) -> bool;
    //fn handle_response(&mut self, res: Response);
}
