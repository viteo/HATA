//! Placement of the sections and cards of a dashboard on a grid.
//!
//! The grid is measured in cells and rows. A column of sections is 12 cells wide,
//! as in the sections view of the web app, and a row is as high as a tile card.
use ratatui::layout::Rect;

use crate::types::app::{Card, RendererKind, Section};

/// The cells in one column of sections.
pub const SECTION_CELLS: u16 = 12;
/// The width of a card as wide as its section.
pub const FULL: u16 = u16::MAX;
/// The lines count in a row.
pub const ROW_LINES: u16 = 6;

/// A rectangle on the grid, in cells and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridRect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Debug, Default)]
pub struct DashboardLayout {
    /// The width of the grid in cells.
    pub width: u16,
    /// The height of the grid in rows.
    pub height: u16,
    pub sections: Vec<GridRect>,
    /// Where each card is, by its index; a heading is a grid line, zero rows high.
    pub cards: Vec<Option<GridRect>>,
}

/// Places rectangles on a grid of a fixed width, each as high up as it fits, then as far left.
struct Skyline {
    /// The first free row of each cell column.
    heights: Vec<u16>,
}

impl Skyline {
    fn new(width: u16) -> Self {
        Self { heights: vec![0; width as usize] }
    }

    fn height(&self) -> u16 {
        self.heights.iter().copied().max().unwrap_or(0)
    }

    /// The highest position where a rectangle `w` cells wide fits.
    fn fit(&self, w: u16) -> (u16, u16) {
        let w = w as usize;
        (0..=self.heights.len() - w)
            .map(|x| (x as u16, self.heights[x..x + w].iter().copied().max().unwrap_or(0)))
            .min_by_key(|&(x, y)| (y, x))
            .unwrap_or((0, 0))
    }

    fn place(&mut self, rect: GridRect) {
        let end = (rect.x + rect.w) as usize;
        self.heights[rect.x as usize..end].fill(rect.y + rect.h);
    }
}

/// The cards of the section placed in a section `width` cells wide, relative to the section,
/// and the height of the section.
fn layout_section(section: &Section, cards: &[Card], width: u16) -> (Vec<(usize, GridRect)>, u16) {
    let mut skyline = Skyline::new(width);
    let mut placed = Vec::new();
    for &i in &section.cards {
        let card = &cards[i];
        let rect = if card.renderer == RendererKind::Heading {
            // a heading labels the line under everything before it
            GridRect { x: 0, y: skyline.height(), w: width, h: 0 }
        } else {
            let w = card.columns.clamp(1, width);
            let (x, y) = skyline.fit(w);
            GridRect { x, y, w, h: card.rows.max(1) }
        };
        skyline.place(rect);
        placed.push((i, rect));
    }
    (placed, skyline.height())
}

/// Places the sections on a grid `columns` sections wide.
pub fn layout(sections: &[Section], cards: &[Card], columns: u16) -> DashboardLayout {
    let width = columns * SECTION_CELLS;
    let mut skyline = Skyline::new(width);
    let mut layout = DashboardLayout {
        width,
        cards: vec![None; cards.len()],
        ..Default::default()
    };
    for section in sections {
        // a section narrower than asked may fit higher up; on a tie the widest wins
        let Some((rect, placed)) = (1..=section.column_span.clamp(1, columns))
            .rev()
            .map(|span| {
                let (placed, h) = layout_section(section, cards, span * SECTION_CELLS);
                let (x, y) = skyline.fit(span * SECTION_CELLS);
                (GridRect { x, y, w: span * SECTION_CELLS, h }, placed)
            })
            .min_by_key(|(rect, _)| rect.y)
        else {
            continue;
        };
        if rect.h == 0 {
            continue;
        }
        skyline.place(rect);
        layout.sections.push(rect);
        for (i, card) in placed {
            layout.cards[i] = Some(GridRect { x: rect.x + card.x, y: rect.y + card.y, ..card });
        }
    }
    layout.height = skyline.height();
    layout
}

/// The mapping of the grid onto the characters of the screen.
pub struct Grid {
    /// The width of the grid in cells.
    pub cells: u16,
    /// The width of the screen in characters.
    pub chars: u16,
}

impl Grid {
    /// The column of the grid line left of the cell.
    fn x(&self, cell: u16) -> u16 {
        (cell as u32 * self.chars.saturating_sub(1) as u32 / self.cells.max(1) as u32) as u16
    }

    /// The characters of the rectangle, its borders included.
    pub fn rect(&self, rect: GridRect) -> Rect {
        let x = self.x(rect.x);
        Rect::new(x, rect.y * ROW_LINES, self.x(rect.x + rect.w) - x + 1, rect.h * ROW_LINES + 1)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// The nearest card in the direction from the card `from`, of the cards `accept` takes.
/// Only the cards wholly past the edge of `from` in the direction count.
pub fn neighbor(layout: &DashboardLayout, from: usize, direction: Direction, accept: impl Fn(usize) -> bool) -> Option<usize> {
    let from_rect = layout.cards.get(from)?.as_ref()?;
    // the edges of a rectangle on an axis
    let xs = |r: &GridRect| (r.x as i32, (r.x + r.w) as i32);
    let ys = |r: &GridRect| (r.y as i32, (r.y + r.h) as i32);
    // how far apart two ranges are; nothing when they overlap, one when they touch
    let apart = |(start, end): (i32, i32), (from_start, from_end): (i32, i32)| (start - from_end).max(from_start - end).max(-1) + 1;
    // doubled, so the center is a whole number
    let center = |r: &GridRect| (2 * r.x as i32 + r.w as i32, 2 * r.y as i32 + r.h as i32);
    let (from_x, from_y) = center(from_rect);
    layout
        .cards
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != from && accept(i))
        .filter_map(|(i, rect)| {
            let rect = rect.as_ref()?;
            // the gap past the edge, and how far beside the card is
            let (gap, beside) = match direction {
                Direction::Up => (ys(from_rect).0 - ys(rect).1, apart(xs(rect), xs(from_rect))),
                Direction::Down => (ys(rect).0 - ys(from_rect).1, apart(xs(rect), xs(from_rect))),
                Direction::Left => (xs(from_rect).0 - xs(rect).1, apart(ys(rect), ys(from_rect))),
                Direction::Right => (xs(rect).0 - xs(from_rect).1, apart(ys(rect), ys(from_rect))),
            };
            // on a tie the nearer center wins, along the direction first,
            // so a heading wins over the cards beyond its line
            let (x, y) = center(rect);
            let (along, across) = match direction {
                Direction::Up | Direction::Down => ((y - from_y).abs(), (x - from_x).abs()),
                Direction::Left | Direction::Right => ((x - from_x).abs(), (y - from_y).abs()),
            };
            (gap >= 0).then_some((i, (gap + 2 * beside, along, across)))
        })
        .min_by_key(|&(_, score)| score)
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(renderer: RendererKind, columns: u16, rows: u16) -> Card {
        let mut card = Card::new("local.test".to_string(), "local".to_string(), String::new());
        card.renderer = renderer;
        card.columns = columns;
        card.rows = rows;
        card
    }

    fn tile() -> Card {
        card(RendererKind::Default, 6, 1)
    }

    fn heading() -> Card {
        card(RendererKind::Heading, FULL, 0)
    }

    #[test]
    fn tiles_fill_a_section_two_by_row() {
        let cards = vec![tile(), tile(), tile()];
        let sections = vec![Section { column_span: 1, cards: vec![0, 1, 2] }];
        let layout = layout(&sections, &cards, 1);
        assert_eq!(layout.cards[0], Some(GridRect { x: 0, y: 0, w: 6, h: 1 }));
        assert_eq!(layout.cards[1], Some(GridRect { x: 6, y: 0, w: 6, h: 1 }));
        assert_eq!(layout.cards[2], Some(GridRect { x: 0, y: 1, w: 6, h: 1 }));
        assert_eq!(layout.height, 2);
    }

    #[test]
    fn heading_is_a_line_under_the_cards_before_it() {
        // a tall card and a short one, then a heading
        let cards = vec![card(RendererKind::NumericTrend, 6, 2), tile(), heading(), tile()];
        let sections = vec![Section { column_span: 1, cards: vec![0, 1, 2, 3] }];
        let layout = layout(&sections, &cards, 1);
        assert_eq!(layout.cards[2], Some(GridRect { x: 0, y: 2, w: 12, h: 0 }));
        assert_eq!(layout.cards[3], Some(GridRect { x: 0, y: 2, w: 6, h: 1 }));
    }

    #[test]
    fn heading_on_top_is_on_the_top_border() {
        let cards = vec![heading(), tile()];
        let sections = vec![Section { column_span: 1, cards: vec![0, 1] }];
        let layout = layout(&sections, &cards, 1);
        assert_eq!(layout.cards[0], Some(GridRect { x: 0, y: 0, w: 12, h: 0 }));
        assert_eq!(layout.cards[1], Some(GridRect { x: 0, y: 0, w: 6, h: 1 }));
    }

    #[test]
    fn section_fills_the_gap_beside_a_shorter_one() {
        // one row, two rows, then a section that fits under the first
        let cards = vec![tile(), tile(), tile(), tile()];
        let sections = vec![
            Section { column_span: 1, cards: vec![0] },
            Section { column_span: 1, cards: vec![1, 2] },
            Section { column_span: 1, cards: vec![3] },
        ];
        let layout = layout(&sections, &cards, 2);
        assert_eq!(layout.sections[2], GridRect { x: 0, y: 1, w: 12, h: 1 });
    }

    #[test]
    fn wide_section_narrows_to_fit_higher() {
        // a wide section of four tiles beside a narrow one goes into the free column, two rows high
        let cards = vec![tile(), tile(), tile(), tile(), tile()];
        let sections = vec![
            Section { column_span: 1, cards: vec![0] },
            Section { column_span: 2, cards: vec![1, 2, 3, 4] },
        ];
        let layout = layout(&sections, &cards, 2);
        assert_eq!(layout.sections[1], GridRect { x: 12, y: 0, w: 12, h: 2 });
    }

    #[test]
    fn wide_section_keeps_its_width_when_narrowing_does_not_help() {
        let cards = vec![tile(), tile()];
        let sections = vec![Section { column_span: 2, cards: vec![0, 1] }];
        let layout = layout(&sections, &cards, 2);
        assert_eq!(layout.sections[0], GridRect { x: 0, y: 0, w: 24, h: 1 });
    }

    #[test]
    fn neighbours_share_the_grid_line() {
        let grid = Grid { cells: 12, chars: 25 };
        let left = grid.rect(GridRect { x: 0, y: 0, w: 6, h: 1 });
        let right = grid.rect(GridRect { x: 6, y: 0, w: 6, h: 1 });
        assert_eq!(left, Rect::new(0, 0, 13, ROW_LINES + 1));
        assert_eq!(right.x, left.right() - 1);
        assert_eq!(right.right(), 25);
    }

    #[test]
    fn arrows_move_to_the_nearest_card() {
        let cards = vec![tile(), tile(), tile()];
        let sections = vec![Section { column_span: 1, cards: vec![0, 1, 2] }];
        let layout = layout(&sections, &cards, 1);
        assert_eq!(neighbor(&layout, 0, Direction::Right, |_| true), Some(1));
        assert_eq!(neighbor(&layout, 0, Direction::Down, |_| true), Some(2));
        assert_eq!(neighbor(&layout, 1, Direction::Down, |_| true), Some(2));
        assert_eq!(neighbor(&layout, 0, Direction::Up, |_| true), None);
    }

    /// The sections of the dashboard-tui dashboard.
    fn water_dashboard() -> (Vec<Card>, Vec<Section>) {
        // 0 1 | 2 3 4:heading 5 6 | 7:heading 8:markdown 9
        let cards = vec![
            tile(), tile(),
            tile(), tile(), card(RendererKind::Heading, FULL, 0), tile(), tile(),
            heading(), card(RendererKind::Markdown, FULL, 1), tile(),
        ];
        let sections = vec![
            Section { column_span: 1, cards: vec![0, 1] },
            Section { column_span: 1, cards: vec![2, 3, 4, 5, 6] },
            Section { column_span: 1, cards: vec![7, 8, 9] },
        ];
        (cards, sections)
    }

    #[test]
    fn arrows_move_around_a_heading() {
        let (cards, sections) = water_dashboard();
        for columns in [1, 3] {
            let layout = layout(&sections, &cards, columns);
            let go = |from, direction| neighbor(&layout, from, direction, |i| i != 7);
            assert_eq!(go(4, Direction::Down), Some(5), "{columns} columns");
            assert_eq!(go(5, Direction::Right), Some(6), "{columns} columns");
            assert_eq!(go(6, Direction::Left), Some(5), "{columns} columns");
            assert_eq!(go(5, Direction::Up), Some(4), "{columns} columns");
            assert_eq!(go(3, Direction::Down), Some(4), "{columns} columns");
            assert_eq!(go(4, Direction::Up), Some(2), "{columns} columns");
        }
    }

    #[test]
    fn arrows_move_through_stacked_sections() {
        let (cards, sections) = water_dashboard();
        let layout = layout(&sections, &cards, 1);
        let go = |from, direction| neighbor(&layout, from, direction, |i| i != 7);
        assert_eq!(go(1, Direction::Down), Some(3));
        assert_eq!(go(0, Direction::Down), Some(2));
        assert_eq!(go(6, Direction::Down), Some(8));
        assert_eq!(go(8, Direction::Down), Some(9));
        assert_eq!(go(9, Direction::Up), Some(8));
        assert_eq!(go(2, Direction::Up), Some(0));
    }

    #[test]
    fn arrows_reach_a_heading_and_a_wide_card() {
        // tile, tile / heading / tile / wide card
        let cards = vec![tile(), tile(), heading(), tile(), card(RendererKind::Markdown, FULL, 1)];
        let sections = vec![Section { column_span: 1, cards: vec![0, 1, 2, 3, 4] }];
        let layout = layout(&sections, &cards, 1);
        assert_eq!(neighbor(&layout, 0, Direction::Down, |_| true), Some(2));
        assert_eq!(neighbor(&layout, 2, Direction::Down, |_| true), Some(3));
        assert_eq!(neighbor(&layout, 3, Direction::Down, |_| true), Some(4));
    }
}
