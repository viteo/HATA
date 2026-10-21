use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Paragraph, Sparkline, Wrap},
};

use crate::types::app::Card;

pub fn render(frame: &mut Frame, area: Rect, card: &Card, block: Block, border_style: Style) {
    let [value_area, graph_area, details_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .areas(area);

    let value = Paragraph::new(Line::from(format!(" {} ", card.state)))
        .alignment(Alignment::Center)
        .style(border_style.fg(Color::White).bold())
        .block(block);
    frame.render_widget(value, value_area);

    if let Some(samples) = &card.samples {
        if samples.len() >= 2 {
            let sparkline_data = normalize_samples(samples);
            let sparkline = Sparkline::default()
                .data(&sparkline_data)
                .style(Style::default().fg(Color::Green));
            frame.render_widget(sparkline, graph_area);
        }
    }

    let action = card.toggle_service().unwrap_or_else(|| "n/a".to_string());
    let details = Paragraph::new(format!("{}\nact: {}", card.domain, action))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    frame.render_widget(details, details_area);
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
