use anyhow::{Context, Result};
use crossterm::{
    event::{EventStream, KeyCode, KeyEventKind, Event as CtEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal, backend::CrosstermBackend, buffer::Buffer, layout::{Alignment, Constraint, Layout, Rect}, style::{Color, Style}, widgets::{Block, Paragraph, StatefulWidget, Widget, Wrap}
};
use std::{io::{self, Stdout}};
use tokio::sync::mpsc;

use crate::types::app::{AppEvent, AppState, Card};

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

pub async fn tui_worker(ui_rx: &mut mpsc::Receiver<AppEvent>, ev_tx: &mpsc::Sender<AppEvent>) -> Result<()> {
    let mut terminal = terminal_setup()?;
    let mut app = AppState::new();
    let mut input = EventStream::new();

    terminal.draw(|f| renderer_cb(f, &mut app))?;

    loop {
        tokio::select! {
            maybe_ev = ui_rx.recv() => {
                if let Some(ev) = maybe_ev {
                    match ev {
                        AppEvent::Snapshot { entities } => {
                            app.entities.extend(entities);
                            app.selected = 0;
                        },
                        AppEvent::EventAdded { entity_id, friendly_name, state } => {
                            if let Some(card) = app.entities.get_mut(&entity_id){
                                card.friendly_name = friendly_name;
                                card.state = state;
                            }
                        }
                        AppEvent::StateChanged { entity_id, state } => {
                            if let Some(card) = app.entities.get_mut(&entity_id){
                                card.state = state;
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
                                app.selected = if app.selected == 0 { app.entities.len() - 1} else { app.selected - 1 };
                            }
                            KeyCode::Down => {
                                app.selected = if app.selected >= app.entities.len() - 1 { 0 } else { app.selected + 1 };
                            }
                            KeyCode::Enter => {
                                if !app.entities.is_empty() {
                                    let (entity_id, _) = app.entities.get_index(app.selected).unwrap();
                                    let service = "toggle".to_string(); // or any other service
                                    let _ = ev_tx.send(AppEvent::CallService { entity_id: entity_id.to_string(), service }).await;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        terminal.draw(|f| renderer_cb(f, &mut app))?;
    }
    
    terminal_restore(&mut terminal)
}

fn renderer_cb(frame: &mut Frame, app: &mut AppState) {
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

    if app.entities.is_empty() {
        return;
    }

    let cols_total = if area.width < CARD_WIDTH {
        1
    } else {
        ((area.width + SPACING) / (CARD_WIDTH + SPACING)) as usize
    };

    let rows_total = (app.entities.len() + cols_total - 1) / cols_total;

    let row_areas = Layout::vertical(vec![Constraint::Length(CARD_HEIGHT); rows_total])
        .spacing(SPACING)
        .split(*area);

    for (row_i, &row_area) in row_areas.iter().enumerate() {
        let start = row_i * cols_total;
        let end = (start + cols_total).min(app.entities.len());
        let row_entities = &app.entities[start..end];

        let col_areas = Layout::horizontal(vec![Constraint::Length(CARD_WIDTH); row_entities.len()])
            .spacing(SPACING)
            .split(row_area);

        for (col_i, &cell_area) in col_areas.iter().enumerate() {
            let card_i = start + col_i;
            if let Some(card) = app.entities.get_index(card_i) {
                let mut is_selected = card_i == app.selected;
                card.1.render(cell_area, frame.buffer_mut(), &mut is_selected);
            }
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

impl StatefulWidget for &Card {
    type State = bool;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_style = if *state == true {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default()
        };

        let block = Block::bordered()
            .title(self.friendly_name.clone())
            .title_bottom(if *state { "^" } else { "" })
            .title_alignment(Alignment::Center)
            .border_style(border_style);

        let content = Paragraph::new(format!("{}\n{}\n{:?}", self.state, self.domain, self.services))
            .block(block)
            .style(border_style)
            .wrap( Wrap { trim: true} );

        content.render(area, buf);
    }
}