//! Link detection in terminal grid coordinates, including wrapped text.

use std::cell::RefCell;
use std::ops::RangeInclusive;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Direction, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::{RegexIter, RegexSearch};
use alacritty_terminal::Term;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLink {
    pub range: RangeInclusive<Point>,
    pub uri: String,
}

// A terminal line can span thousands of history rows. Bound pointer work,
// measured in cells, and compile the core's grid-aware matcher only once.
const MAX_LINK_CELLS: usize = 4096;
thread_local! {
    static URL_REGEX: RefCell<Option<RegexSearch>> =
        RefCell::new(RegexSearch::new(r#"https?://[^\s<>"'`]+"#).ok());
}

pub(super) fn link_at<T: EventListener>(term: &Term<T>, point: Point) -> Option<TerminalLink> {
    let grid = term.grid();
    if point.column.0 >= term.columns()
        || point != point.grid_clamp(term, Boundary::Grid)
        || grid[point].flags.contains(Flags::HIDDEN)
    {
        return None;
    }
    if let Some(link) = grid[point].hyperlink() {
        if !crate::util::is_web_url(link.uri()) {
            return None;
        }
        let mut start = point;
        let mut end = point;
        for _ in 0..MAX_LINK_CELLS {
            let previous = start.sub(term, Boundary::Grid, 1);
            if previous == start || grid[previous].hyperlink().as_ref() != Some(&link) {
                break;
            }
            start = previous;
        }
        for _ in 0..MAX_LINK_CELLS {
            let next = end.add(term, Boundary::Grid, 1);
            if next == end || grid[next].hyperlink().as_ref() != Some(&link) {
                break;
            }
            end = next;
        }
        return Some(TerminalLink {
            range: start..=end,
            uri: link.uri().to_owned(),
        });
    }

    let start = term.line_search_left(point);
    let end = term.line_search_right(point);
    let cells = (end.line.0 - start.line.0) as usize * term.columns() + term.columns();
    if cells > MAX_LINK_CELLS {
        return None;
    }
    URL_REGEX.with(|regex| {
        let mut regex = regex.borrow_mut();
        let regex = regex.as_mut()?;
        let range = RegexIter::new(start, end, Direction::Right, term, regex)
            .find(|range| range.contains(&point))?;
        let mut cell = *range.start();
        loop {
            if grid[cell].flags.contains(Flags::HIDDEN) {
                return None;
            }
            if cell == *range.end() {
                break;
            }
            cell = cell.add(term, Boundary::Grid, 1);
        }
        let mut uri = term.bounds_to_string(*range.start(), *range.end());
        let mut end = *range.end();
        // Ignore sentence punctuation and unmatched prose delimiters without
        // removing balanced parentheses from real URL paths.
        while let Some(last) = uri.chars().last() {
            let trim = matches!(last, '.' | ',' | ';' | ':' | '!' | '?')
                || [('(', ')'), ('[', ']'), ('{', '}')]
                    .iter()
                    .any(|&(open, close)| {
                        last == close && uri.matches(close).count() > uri.matches(open).count()
                    });
            if !trim {
                break;
            }
            uri.pop();
            end = end.sub(term, Boundary::Grid, 1);
        }
        let range = *range.start()..=end;
        (range.contains(&point) && crate::util::is_web_url(&uri))
            .then_some(TerminalLink { range, uri })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{PtyHandle, TerminalSession};
    use alacritty_terminal::index::{Column, Line};

    #[test]
    fn terminal_links_preserve_wrapped_urls_and_osc_targets_but_reject_unsafe_schemes() {
        let (pty, _) = PtyHandle::new_for_test();
        let (tx, _) = std::sync::mpsc::channel();
        let mut session = TerminalSession::new(0, 3, 16, pty, tx);
        session.apply_bytes(b"(https://example.com/a(b)).\r\nnext\r\nlast");
        let point = Point::new(Line(-1), Column(10));
        let link = session.link_at(point).unwrap();
        assert_eq!(link.uri, "https://example.com/a(b)");
        assert_eq!(
            link.range,
            Point::new(Line(-1), Column(1))..=Point::new(Line(0), Column(8))
        );
        assert!(session.link_at(Point::new(Line(0), Column(9))).is_none());
        session.clear();
        session.apply_bytes(b"\x1b[H\x1b]8;;https://example.com/target\x1b\\label\x1b]8;;\x1b\\");
        assert_eq!(
            session.link_at(Point::default()).unwrap().uri,
            "https://example.com/target"
        );
        session.clear();
        session.apply_bytes(b"\x1b[H\x1b]8;;file:///tmp/no\x1b\\label\x1b]8;;\x1b\\");
        assert!(session.link_at(Point::default()).is_none());
        session.clear();
        session.apply_bytes(b"\x1b[H\x1b[8mhttps://e.test\x1b[0m");
        assert!(session.link_at(Point::default()).is_none());
        for uri in [
            "javascript:alert(1)",
            "file:///tmp/a",
            "https://",
            "https://e.test/\narg",
        ] {
            assert!(!crate::util::is_web_url(uri));
        }
        assert!(crate::util::is_web_url("http://localhost:3000/path"));
    }
}
