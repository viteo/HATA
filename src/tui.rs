use anyhow::{Context, Result};
use crossterm::{
    event::{EventStream, KeyCode, KeyEventKind, Event as CtEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal, backend::CrosstermBackend, layout::{Constraint, Layout, Rect}, style::{Modifier, Style}, widgets::{Block, Borders, List, ListItem, Paragraph}
};
use std::{io::{self, Stdout}};
use tokio::sync::mpsc;

use crate::app::{AppEvent, AppState};

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
    terminal.show_cursor().ok();
    Ok(())
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
                            app.entities.extend(entities.into_iter());
                            app.selected.select(Some(0));
                        }
                        AppEvent::StateChanged { entity_id, state } => {
                            app.entities.insert(entity_id, state);
                        }
                        AppEvent::Status(s) => app.status = s,
                        AppEvent::Error(e) => app.last_error = Some(e),
                        _ => {},
                    }
                    terminal.draw(|f| renderer_cb(f, &mut app))?;
                }
            }
            maybe_in = input.next() => {
                let Some(Ok(evt)) = maybe_in else { continue; };
                if let CtEvent::Key(key) = evt {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Up => {
                                if let Some(i) = app.selected.selected() {
                                    app.selected.select(Some(if i == 0 { app.entities.len() - 1} else { i - 1 }));
                                }
                            }
                            KeyCode::Down => {
                                if let Some(i) = app.selected.selected() {
                                    app.selected.select(Some(if i >= app.entities.len() - 1 { 0 } else { i + 1 }));
                                }
                            }
                            KeyCode::Enter => {
                                if !app.entities.is_empty() {
                                    let (entity_id, _) = app.entities.get_index(app.selected.selected().unwrap()).unwrap();
                                    let service = "toggle".to_string(); // or any other service
                                    let _ = ev_tx.send(AppEvent::CallService { entity_id: entity_id.to_string(), service }).await;
                                }
                            }
                            _ => {}
                        }
                        terminal.draw(|f| renderer_cb(f, &mut app))?;
                    }
                }
            }
        }
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
    let items: Vec<ListItem> = app
        .entities
        .iter()
        .map(|id| {
            ListItem::new(format!("{}: {}", id.0, id.1))
        })
        .collect();

    let mut list = List::new(items)
        .block(Block::bordered()
            .borders(Borders::ALL)
            // .merge_borders(MergeStrategy::Exact)
            .title("Entities"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    if !app.entities.is_empty() {
        list = list.highlight_symbol(">> ");
    }

    frame.render_stateful_widget(list, *area, &mut app.selected);
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