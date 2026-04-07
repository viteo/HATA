use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph, Wrap},
};

use crate::types::app::Card;

pub fn render(frame: &mut Frame, area: Rect, card: &Card, block: Block, border_style: Style) {
    let widget = Paragraph::new(Line::from(card.state.clone()))
        .alignment(Alignment::Center)
        .block(block)
        .style(border_style.fg(Color::Cyan).bold())
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}
