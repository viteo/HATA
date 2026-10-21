use anyhow::{Context, Result};
use crossterm::{
    event::{EventStream, KeyCode, KeyEventKind, Event as CtEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal, backend::CrosstermBackend, layout::{Constraint, Layout, Rect}, widgets::{Block, Paragraph}
};
use std::{io::{self, Stdout}};
use tokio::sync::mpsc;

use crate::renderers::render_card;
use crate::types::app::{AppEvent, AppState};

pub fn terminal_setup() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("Failed to create terminal")
}

pub fn terminal_restore(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to restore cursor")
}

pub async fn tui_worker(ui_rx: &mut mpsc::Receiver<AppEvent>, cmd_tx: &mpsc::Sender<AppEvent>) -> Result<()> {
    let mut terminal = terminal_setup()?;
    let mut app = AppState::new();
    let mut input = EventStream::new();

    terminal.clear()?;
    terminal.draw(|f| draw(f, &mut app))?;

    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = tick.tick() => {}
            maybe_ev = ui_rx.recv() => {
                if let Some(ev) = maybe_ev {
                    match ev {
                        AppEvent::Snapshot { cards } => {
                            app.cards = cards;
                            app.selected = 0;
                        },
                        AppEvent::EntityAdded { entity_id, name, state } => {
                            for card in app.cards_for_entity(&entity_id) {
                                if card.name.is_empty() {
                                    card.name = name.clone();
                                }
                                card.state = state.clone();
                                card.push_sample();
                            }
                        }
                        AppEvent::StateChanged { entity_id, state } => {
                            for card in app.cards_for_entity(&entity_id) {
                                card.state = state.clone();
                                card.push_sample();
                            }
                        }
                        AppEvent::Status(s) => app.status = s,
                        AppEvent::Error(e) => app.last_error = Some(e),
                        _ => {},
                    }
                }
            }
            maybe_in = input.next() => {
                let Some(Ok(evt)) = maybe_in else { continue; };
                if let CtEvent::Key(key) = evt {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Up => {
                                app.selected = if app.selected == 0 { app.cards.len() - 1} else { app.selected - 1 };
                            }
                            KeyCode::Down => {
                                app.selected = if app.selected >= app.cards.len() - 1 { 0 } else { app.selected + 1 };
                            }
                            KeyCode::Enter => {
                                if !app.cards.is_empty() {
                                    let card = &app.cards[app.selected];
                                    let entity_id = card.entity_id.clone();
                                    let action = card.toggle_service();
                                    let data = card.tap_action_data();
                                    if let Some(service) = action {
                                        let _ = cmd_tx.send(AppEvent::CallService { entity_id, service, data }).await;
                                    } else {
                                        app.last_error = Some(format!("No supported service for {}", entity_id));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        terminal.draw(|f| draw(f, &mut app))?;
    }

    terminal_restore(&mut terminal)
}

fn draw(frame: &mut Frame, app: &mut AppState) {
    let [header ,body, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        // .spacing(Spacing::Overlap(1))
        .areas(frame.area());

    draw_header(frame, &app, &header);
    draw_body(frame, app, &body);
    draw_footer(frame, &app, &footer);
}

fn draw_header(frame: &mut Frame, app: &AppState, area: &Rect) {
    let header = Paragraph::new(format!("App status: {}", app.status))
    .block(Block::bordered()
        // .merge_borders(MergeStrategy::Exact)
        .title(app.title));

    frame.render_widget(header, *area);
}

fn draw_body(frame: &mut Frame, app: &mut AppState, area: &Rect) {
    const CARD_WIDTH: u16 = 28;
    const CARD_HEIGHT: u16 = 10;
    const SPACING: u16 = 0;

    if app.cards.is_empty() {
        return;
    }

    let cols_total = if area.width < CARD_WIDTH {
        1
    } else {
        ((area.width + SPACING) / (CARD_WIDTH + SPACING)) as usize
    };

    let rows_total = (app.cards.len() + cols_total - 1) / cols_total;

    let row_areas = Layout::vertical(vec![Constraint::Length(CARD_HEIGHT); rows_total])
        .spacing(SPACING)
        .split(*area);

    for (row_i, &row_area) in row_areas.iter().enumerate() {
        let start = row_i * cols_total;
        let end = (start + cols_total).min(app.cards.len());
        let row_cards = &app.cards[start..end];

        let col_areas = Layout::horizontal(vec![Constraint::Length(CARD_WIDTH); row_cards.len()])
            .spacing(SPACING)
            .split(row_area);

        for (col_i, &cell_area) in col_areas.iter().enumerate() {
            let card_i = start + col_i;
            render_card(frame, cell_area, &app.cards[card_i], card_i == app.selected);
        }
    }
}

fn draw_footer(frame: &mut Frame, app: &AppState, area: &Rect) {
    let footer = Paragraph::new(if let Some(err) = &app.last_error {
        format!("q: quit | ↑/↓: scroll | Error -> {err}")
    } else { "q: quit | ↑/↓: scroll".to_string()}
    // ).block(
        // Block::bordered()
            // .merge_borders(MergeStrategy::Exact)
            // .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
    );

    frame.render_widget(footer, *area);
}
