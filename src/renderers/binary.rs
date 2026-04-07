use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Paragraph, Wrap},
};

use crate::types::app::Card;

pub fn render(frame: &mut Frame, area: Rect, card: &Card, block: Block, border_style: Style) {
    let (value_color, label) = match card.state.as_str() {
        "on" | "open" | "home" | "detected" => (Color::Green, "ON"),
        "off" | "closed" | "not_home" | "clear" => (Color::DarkGray, "OFF"),
        _ => (Color::Yellow, card.state.as_str()),
    };
    let action = card.preferred_action().unwrap_or_else(|| "n/a".to_string());
    let widget = Paragraph::new(format!("{}\n{}\nact: {}", label, card.domain, action))
        .alignment(Alignment::Center)
        .block(block)
        .style(border_style.fg(value_color).bold())
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}
