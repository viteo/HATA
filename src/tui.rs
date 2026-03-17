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
use std::io::{self, Stdout};
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

    loop {
        terminal.draw(|frame| {
            let [header ,body, footer] = Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1),])
                // .spacing(Spacing::Overlap(1))
                .areas(frame.area());

            draw_header(frame, &app, &header);
            draw_body(frame, &mut app, &body);
            draw_footer(frame, &app, &footer);
        })?;

        tokio::select! {
            maybe_ev = ui_rx.recv() => {
                if let Some(ev) = maybe_ev {
                    match ev {
                        AppEvent::Snapshot { title, views, entities } => {
                            app.title = title;
                            app.views = views;
                            app.states = entities.iter().map(|(id, st)| (id.clone(), st.clone())).collect();
                            app.entities = entities.into_iter().map(|(id, _)| id).collect();
                            app.entities.sort();
                            app.selected = app.selected.min(app.entities.len().saturating_sub(1));
                        }
                        AppEvent::StateChanged { entity_id, state } => {
                            app.states.insert(entity_id, state);
                        }
                        AppEvent::Status(s) => app.status = s,
                        AppEvent::Error(e) => app.last_error = Some(e),
                        _ => {},
                    }
                } else {
                    // Background task ended; keep UI running so user can see status/error.
                    if app.status == "Connected" {
                        app.status = "Disconnected".to_string();
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
                                if app.selected > 0 { app.selected -= 1; }
                            }
                            KeyCode::Down => {
                                if app.selected + 1 < app.entities.len() { app.selected += 1; }
                            }
                            KeyCode::Enter => {
                                if !app.entities.is_empty() {
                                    let entity_id = app.entities[app.selected].clone();
                                    let service = "toggle".to_string(); // or any other service
                                    let _ = ev_tx.send(AppEvent::CallService { entity_id, service }).await;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    
    terminal_restore(&mut terminal)
}

fn draw_header(frame: &mut Frame, app: &AppState, area: &Rect) {
    let header = Paragraph::new(format!(
        "Views: {}  |  {}",
        app.views, app.status
    ))
    .block(Block::bordered()
        // .merge_borders(MergeStrategy::Exact)
        .title(app.title.clone()));

    frame.render_widget(header, *area);
}

fn draw_body(frame: &mut Frame, app: &mut AppState, area: &Rect) {
    let items: Vec<ListItem> = app
        .entities
        .iter()
        .map(|id| {
            let st = app.states.get(id).map(String::as_str).unwrap_or("<unknown>");
            ListItem::new(format!("{id}: {st}"))
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

    let selected = if app.entities.is_empty() {
        None
    } else {
        Some(app.selected.min(app.entities.len().saturating_sub(1)))
    };
    app.list_state.select(selected);
    frame.render_stateful_widget(list, *area, &mut app.list_state);
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