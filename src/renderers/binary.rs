use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget},
};

use super::named;
use crate::types::app::Card;

pub fn render(buf: &mut Buffer, area: Rect, card: &Card, border_style: Style) {
    let (value_color, label) = match card.state.as_str() {
        "on" | "open" | "home" | "detected" => (Color::Green, "ON"),
        "off" | "closed" | "not_home" | "clear" => (Color::DarkGray, "OFF"),
        _ => (Color::Yellow, card.state.as_str()),
    };
    let action = card.toggle_service().unwrap_or_else(|| "n/a".to_string());
    let widget = Paragraph::new(named(card, format!("{}\n{}\nact: {}", label, card.domain, action)))
        .alignment(Alignment::Center)
        .style(border_style.fg(value_color).bold());
    widget.render(area, buf);
}
