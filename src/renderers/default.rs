use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget},
};

use super::named;
use crate::types::app::Card;

pub fn render(buf: &mut Buffer, area: Rect, card: &Card, border_style: Style) {
    let action = card.toggle_service().unwrap_or_else(|| "n/a".to_string());
    let widget = Paragraph::new(named(card, format!("{}\n{}\nact: {}", card.state, card.domain, action)))
        .alignment(Alignment::Center)
        .style(border_style);
    widget.render(area, buf);
}
