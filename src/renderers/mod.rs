mod binary;
mod clock;
mod default;
mod numeric_trend;
mod markdown;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    symbols::merge::MergeStrategy,
    widgets::{Block, Widget},
};

use crate::types::app::{Card, RendererKind};

pub fn render_card(buf: &mut Buffer, area: Rect, card: &Card, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default()
    };

    // a heading is drawn on its grid line by the dashboard
    if card.renderer == RendererKind::Heading {
        return;
    }

    // the borders are shared with the neighbouring cards,
    // so the renderers only draw inside them
    let block = Block::bordered()
        .merge_borders(MergeStrategy::Fuzzy)
        .border_style(border_style);
    let inner = block.inner(area);
    block.render(area, buf);

    match card.renderer {
        RendererKind::Clock => clock::render(buf, inner, card, border_style),
        RendererKind::Binary => binary::render(buf, inner, card, border_style),
        RendererKind::NumericTrend => numeric_trend::render(buf, inner, card, border_style),
        RendererKind::Markdown => markdown::render(buf, inner, card, border_style),
        RendererKind::Heading => {}
        RendererKind::Default => default::render(buf, inner, card, border_style),
    }
}

/// The text under the name of the card, if it has one.
fn named(card: &Card, text: String) -> String {
    if card.name.is_empty() {
        text
    } else {
        format!("{}\n{}", card.name, text)
    }
}
