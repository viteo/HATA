use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget},
};

use super::named;
use crate::types::app::Card;

// set to false to hide the seconds.
const SHOW_SECONDS: bool = true;

pub fn render(buf: &mut Buffer, area: Rect, card: &Card, border_style: Style) {
    let now = chrono::Local::now();
    let time = if SHOW_SECONDS {
        now.format("%H:%M:%S").to_string()
    } else {
        now.format("%H:%M").to_string()
    };

    let widget = Paragraph::new(named(card, time))
        .alignment(Alignment::Center)
        .style(border_style.fg(Color::Cyan).bold());
    widget.render(area, buf);
}
