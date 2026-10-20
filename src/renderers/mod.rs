mod binary;
mod clock;
mod default;
mod numeric_trend;
mod text;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::Block,
};

use crate::types::app::{Card, RendererKind};

pub fn render_card(frame: &mut Frame, area: Rect, card: &Card, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default()
    };

    let block = Block::bordered()
        .title(card.friendly_name.clone())
        .title_bottom(if is_selected { "^" } else { "" })
        .title_alignment(Alignment::Center)
        .border_style(border_style);

    match card.renderer {
        RendererKind::Clock => clock::render(frame, area, card, block, border_style),
        RendererKind::Binary => binary::render(frame, area, card, block, border_style),
        RendererKind::NumericTrend => {
            numeric_trend::render(frame, area, card, block, border_style)
        }
        RendererKind::Markdown => text::render(frame, area, card, block, border_style),
        RendererKind::Default => default::render(frame, area, card, block, border_style),
    }
}
