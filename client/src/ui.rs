use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::{Frame, Terminal};

use crate::{MessageType, ServerMessage, State, UserData};

fn render_message<'a>(message: &'a ServerMessage) -> Vec<Line<'a>> {
    match &message.ty {
        MessageType::Public { msg, from } => vec![Line::from(vec![
            Span::styled(
                format!("[{}] ", message.timestamp.format("%H:%M")),
                Style::new().yellow().bold(),
            ),
            Span::styled(format!("{from}: "), Style::new().bold()),
            Span::from(msg),
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
        MessageType::Private { from, msg } => {
            vec![Line::from(vec![
                Span::styled(
                    format!("[{}] ", message.timestamp.format("%H:%M")),
                    Style::new().green().bold(),
                ),
                Span::styled(format!("{from}: "), Style::new().green().bold()),
                Span::styled(msg, Style::new().green().bold()),
            ])]
        }
    }
}

fn render_message_area(f: &mut Frame, state: &State, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Min(1)])
        .split(area);

    let max_messages = chunks[0].height - 2;

    // render another area for debug messages!

    let messages = match state.current_tab {
        None => vec![],
        Some(ref room_name) => match state.room_messages.get(room_name) {
            Some(messages) => messages
                .iter()
                .map(|msg| render_message(msg))
                .flatten()
                .collect::<Vec<_>>(),
            None => panic!("Unkown room name {room_name}"),
        },
    };

    let n_messages = messages.len() as u16;

    let offset = u16::max(n_messages.saturating_sub(max_messages), 0);

    let para = Paragraph::new(messages)
        .scroll((offset, 0))
        .block(Block::default().borders(Borders::ALL));

    let debug_messages = Paragraph::new(
        state
            .debug_messages
            .iter()
            .map(|msg| Line::from(msg.as_str()))
            .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::ALL));

    if state.show_debug {
        f.render_widget(para, chunks[0]);
        f.render_widget(debug_messages, chunks[1]);
    } else {
        f.render_widget(para, area);
    }
}

pub fn ui(f: &mut Frame, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Max(1), Constraint::Min(10), Constraint::Max(3)])
        .split(f.size());

    let current_room = match &state.current_tab {
        Some(name) => name,
        None => "NONE",
    };

    let status_line = match &state.user_data {
        Some(UserData { username }) => Paragraph::new(Line::from(vec![
            Span::styled(username, Style::new().bold()),
            Span::from(" in #"),
            Span::styled(current_room, Style::new().yellow()),
        ])),
        None => Paragraph::new("..."),
    };

    f.render_widget(status_line, chunks[0]);
    render_message_area(f, state, chunks[1]);
    f.render_widget(state.textarea.widget(), chunks[2]);
}
