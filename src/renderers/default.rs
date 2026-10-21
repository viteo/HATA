use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Block, Paragraph, Wrap},
};

use crate::types::app::Card;

pub fn render(frame: &mut Frame, area: Rect, card: &Card, block: Block, border_style: Style) {
    let action = card.toggle_service().unwrap_or_else(|| "n/a".to_string());
    let widget = Paragraph::new(format!("{}\n{}\nact: {}", card.state, card.domain, action))
        .alignment(Alignment::Center)
        .block(block)
        .style(border_style)
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}
