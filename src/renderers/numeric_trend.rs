use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Sparkline, Widget, Wrap},
};

use super::named;
use crate::types::app::Card;

pub fn render(buf: &mut Buffer, area: Rect, card: &Card, border_style: Style) {
    let [value_area, graph_area, details_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    let value = Paragraph::new(named(card, card.state.clone()))
        .alignment(Alignment::Center)
        .style(border_style.fg(Color::White).bold());
    value.render(value_area, buf);

    if let Some(samples) = &card.samples {
        if samples.len() >= 2 {
            let sparkline_data = normalize_samples(samples);
            let sparkline = Sparkline::default()
                .data(&sparkline_data)
                .style(Style::default().fg(Color::Green));
            sparkline.render(graph_area, buf);
        }
    }

    let action = card.toggle_service().unwrap_or_else(|| "n/a".to_string());
    let details = Paragraph::new(format!("{}\nact: {}", card.domain, action))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    details.render(details_area, buf);
}

fn normalize_samples(samples: &[f32]) -> Vec<u64> {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in samples {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    if (max - min).abs() < f32::EPSILON {
        return vec![50; samples.len()];
    }
    samples
        .iter()
        .map(|v| (((v - min) / (max - min)) * 100.0).round() as u64)
        .collect()
}
