use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget, Wrap},
};

use super::named;
use crate::types::app::Card;

pub fn render(buf: &mut Buffer, area: Rect, card: &Card, border_style: Style) {
    let text = named(card, plain_text(&card.state));

    let widget = Paragraph::new(text)
        .alignment(Alignment::Left)
        .style(border_style.fg(Color::White))
        .wrap(Wrap { trim: true });
    widget.render(area, buf);
}

/// Markdown as plain text, a terminal has no markdown.
fn plain_text(markdown: &str) -> String {
    markdown
        .lines()
        .map(plain_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn plain_line(line: &str) -> String {
    let line = line.trim_start();
    // heading
    let line = line.strip_prefix('#').unwrap_or(line).trim_start();
    // list item
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .unwrap_or(line);
    // emphasis and code
    let line = line.replace("**", "").replace('*', "").replace('`', "");
    // [text](url) -> text
    plain_links(&line)
}

/// strip urls
fn plain_links(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        let Some(bracket) = rest[start..].find(']') else {
            break;
        };
        out.push_str(&rest[start + 1..start + bracket]);
        let after = &rest[start + bracket + 1..];
        // the url of the link, if there is one
        rest = match after.find(')') {
            Some(url_end) => &after[url_end + 1..],
            None => after,
        };
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::plain_text;

    #[test]
    fn markdown_becomes_plain_text() {
        assert_eq!(plain_text("# Title"), "Title");
        assert_eq!(plain_text("**bold** and *italic*"), "bold and italic");
        assert_eq!(plain_text("- an item"), "an item");
        assert_eq!(plain_text("`code`"), "code");
        assert_eq!(plain_text("[home](http://example.com)"), "home");
    }
}
