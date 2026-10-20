use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph, Wrap},
};

use crate::types::app::Card;

// set to false to hide the seconds.
const SHOW_SECONDS: bool = true;

pub fn render(frame: &mut Frame, area: Rect, _card: &Card, block: Block, border_style: Style) {
    let now = chrono::Local::now();
    let time = if SHOW_SECONDS {
        now.format("%H:%M:%S").to_string()
    } else {
        now.format("%H:%M").to_string()
    };

    let widget = Paragraph::new(Line::from(time))
        .alignment(Alignment::Center)
        .block(block)
        .style(border_style.fg(Color::Cyan).bold())
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}
