use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream, KeyCode, KeyEventKind, MouseButton, MouseEventKind, Event as CtEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal, backend::CrosstermBackend, buffer::Buffer, layout::{Constraint, Layout, Position, Rect},
    style::{Color, Style}, symbols::merge::MergeStrategy,
    widgets::{Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};
use std::{io::{self, Stdout}};
use tokio::sync::mpsc;

use crate::layout::{self, DashboardLayout, Direction, Grid};
use crate::renderers::render_card;
use crate::touch::Touchscreen;
use crate::types::app::{AppEvent, AppState, RendererKind};

pub fn terminal_setup() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    execute!(stdout, EnableMouseCapture).context("Failed to enable mouse capture")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("Failed to create terminal")
}

pub fn terminal_restore(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), DisableMouseCapture).context("Failed to disable mouse capture")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to restore cursor")
}

pub async fn tui_worker(ui_rx: &mut mpsc::Receiver<AppEvent>, cmd_tx: &mpsc::Sender<AppEvent>) -> Result<()> {
    let mut terminal = terminal_setup()?;
    let mut app = AppState::new();
    let mut input = EventStream::new();
    let mut touchscreen = Touchscreen::open();

    terminal.clear()?;
    terminal.draw(|f| draw(f, &mut app))?;

    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = tick.tick() => {}
            maybe_ev = ui_rx.recv() => {
                if let Some(ev) = maybe_ev {
                    match ev {
                        AppEvent::Snapshot { cards, sections } => {
                            app.selected = cards.iter().position(|card| card.is_focusable()).unwrap_or(0);
                            app.cards = cards;
                            app.sections = sections;
                            app.scroll = 0;
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
                match evt {
                    CtEvent::Key(key) if key.kind == KeyEventKind::Press => {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Up => select_neighbor(&mut app, Direction::Up),
                            KeyCode::Down => select_neighbor(&mut app, Direction::Down),
                            KeyCode::Left => select_neighbor(&mut app, Direction::Left),
                            KeyCode::Right => select_neighbor(&mut app, Direction::Right),
                            KeyCode::Tab => select_next(&mut app, true),
                            KeyCode::BackTab => select_next(&mut app, false),
                            KeyCode::Enter => activate(&mut app, cmd_tx).await,
                            _ => {}
                        }
                    }
                    CtEvent::Mouse(mouse) => {
                        match mouse.kind {
                            MouseEventKind::Down(MouseButton::Left) => activate_at(&mut app, cmd_tx, mouse.column, mouse.row).await,
                            MouseEventKind::ScrollUp => select_neighbor(&mut app, Direction::Up),
                            MouseEventKind::ScrollDown => select_neighbor(&mut app, Direction::Down),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            Some((column, row)) = async {
                match touchscreen.as_mut() {
                    Some(touchscreen) => touchscreen.next_tap().await,
                    None => std::future::pending().await,
                }
            } => activate_at(&mut app, cmd_tx, column, row).await,
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

/// Whether the card can be selected where it is drawn.
fn is_selectable(app: &AppState, i: usize) -> bool {
    app.cards[i].is_focusable() && app.layout.cards.get(i).is_some_and(|rect| rect.is_some())
}

fn select_neighbor(app: &mut AppState, direction: Direction) {
    if let Some(i) = layout::neighbor(&app.layout, app.selected, direction, |i| is_selectable(app, i)) {
        app.selected = i;
    }
}

/// Calls the service of the selected card.
async fn activate(app: &mut AppState, cmd_tx: &mpsc::Sender<AppEvent>) {
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

/// Selects the card under a tap or a click and acts on it at once, as in the web app.
async fn activate_at(app: &mut AppState, cmd_tx: &mpsc::Sender<AppEvent>, column: u16, row: u16) {
    if let Some(i) = card_at(app, column, row) {
        app.selected = i;
        activate(app, cmd_tx).await;
    }
}

/// The selectable card drawn at the position on the screen.
fn card_at(app: &AppState, column: u16, row: u16) -> Option<usize> {
    let area = app.dashboard_area;
    if !area.contains(Position::new(column, row)) {
        return None;
    }
    let grid = Grid { cells: app.layout.width, chars: area.width.saturating_sub(1) };
    let position = Position::new(column - area.x, row - area.y + app.scroll);
    app.layout
        .cards
        .iter()
        .enumerate()
        .filter(|&(i, rect)| is_selectable(app, i) && rect.is_some_and(|rect| grid.rect(rect).contains(position)))
        // a heading is on a line shared with the cards around it
        .min_by_key(|&(i, _)| app.cards[i].renderer != RendererKind::Heading)
        .map(|(i, _)| i)
}

/// Selects the next card in the order of the dashboard, or the previous one.
fn select_next(app: &mut AppState, forward: bool) {
    let count = app.cards.len();
    for step in 1..count {
        let i = if forward { (app.selected + step) % count } else { (app.selected + count - step) % count };
        if is_selectable(app, i) {
            app.selected = i;
            return;
        }
    }
}

/// The narrowest column of sections, in characters.
const MIN_SECTION_CHARS: u16 = 26;

fn draw_body(frame: &mut Frame, app: &mut AppState, area: &Rect) {
    app.dashboard_area = *area;
    if app.cards.is_empty() {
        return;
    }

    // the last column is for the scrollbar
    let width = area.width.saturating_sub(1);
    let columns = (width.saturating_sub(1) / MIN_SECTION_CHARS).max(1);
    app.layout = layout::layout(&app.sections, &app.cards, columns);
    let grid = Grid { cells: app.layout.width, chars: width };

    let canvas = draw_dashboard(app, &grid);
    let height = canvas.area.height;

    // keep the selected card on the screen
    if let Some(Some(selected)) = app.layout.cards.get(app.selected) {
        let rect = grid.rect(*selected);
        if rect.y < app.scroll {
            app.scroll = rect.y;
        } else if rect.bottom() > app.scroll + area.height {
            app.scroll = rect.bottom().saturating_sub(area.height);
        }
    }
    app.scroll = app.scroll.min(height.saturating_sub(area.height));

    let buf = frame.buffer_mut();
    for y in 0..area.height.min(height - app.scroll) {
        for x in 0..width {
            buf[(area.x + x, area.y + y)] = canvas[(x, app.scroll + y)].clone();
        }
    }

    if height > area.height {
        let mut state = ScrollbarState::new(height.saturating_sub(area.height) as usize)
            .position(app.scroll as usize)
            .viewport_content_length(area.height as usize);
        frame.render_stateful_widget(Scrollbar::new(ScrollbarOrientation::VerticalRight), *area, &mut state);
    }
}

/// The whole dashboard, as high as it is.
fn draw_dashboard(app: &AppState, grid: &Grid) -> Buffer {
    let DashboardLayout { height, sections, cards, .. } = &app.layout;
    let mut canvas = Buffer::empty(Rect::new(0, 0, grid.chars, height * layout::ROW_LINES + 1));
    let highlight = Style::default().fg(Color::Yellow).bold();
    let headings = || {
        cards
            .iter()
            .enumerate()
            .filter_map(|(i, rect)| Some((i, grid.rect((*rect)?))))
            .filter(|(i, _)| app.cards[*i].renderer == RendererKind::Heading)
    };

    for (i, rect) in cards.iter().enumerate() {
        if let Some(rect) = rect {
            render_card(&mut canvas, grid.rect(*rect), &app.cards[i], i == app.selected);
        }
    }
    // a heading inside a section is a line across it
    for (_, rect) in headings() {
        for x in rect.left() + 1..rect.right().saturating_sub(1) {
            canvas[(x, rect.y)].merge_symbol("─", MergeStrategy::Fuzzy);
        }
    }
    for rect in sections {
        Block::bordered()
            .border_type(BorderType::Double)
            .merge_borders(MergeStrategy::Exact)
            .render(grid.rect(*rect), &mut canvas);
    }
    for (i, rect) in headings() {
        let card = &app.cards[i];
        let label = if card.tap_action.is_some() { format!(" {} ► ", card.name) } else { format!(" {} ", card.name) };
        let style = if i == app.selected { highlight } else { Style::default().fg(Color::Reset).bold() };
        canvas.set_stringn(rect.x + 2, rect.y, label, rect.width.saturating_sub(4) as usize, style);
    }

    // the borders of the selected card are shared, so they are colored last
    if let Some(Some(selected)) = cards.get(app.selected)
        && selected.h > 0
    {
        let rect = grid.rect(*selected);
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                let border = y == rect.top() || y == rect.bottom() - 1 || x == rect.left() || x == rect.right() - 1;
                if !border || !canvas.area.contains(Position::new(x, y)) {
                    continue;
                }
                // the labels on the border keep their color
                let cell = &mut canvas[(x, y)];
                if cell.symbol().chars().all(|c| ('\u{2500}'..='\u{257f}').contains(&c)) {
                    cell.set_style(highlight);
                }
            }
        }
    }
    canvas
}

fn draw_footer(frame: &mut Frame, app: &AppState, area: &Rect) {
    let footer = Paragraph::new(if let Some(err) = &app.last_error {
        format!("q: quit | arrows: move | Tab: next | Enter: act | Error -> {err}")
    } else { "q: quit | arrows: move | Tab: next | Enter: act".to_string()}
    // ).block(
        // Block::bordered()
            // .merge_borders(MergeStrategy::Exact)
            // .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
    );

    frame.render_widget(footer, *area);
}
