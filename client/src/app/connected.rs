use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::client::TaskSpawner;

use super::AppState;

pub struct Connected {
    pub username: String,
}

impl AppState for Connected {
    fn render(&self, f: &mut Frame) {
        todo!()
    }

    fn input(&mut self, key: KeyEvent, spawner: &mut TaskSpawner) -> bool {
        todo!()
    }
}
