use std::{sync::atomic::{AtomicU64, Ordering}, cmp};

use itertools::Itertools;
use ratatui::{prelude::*, widgets::{Widget, Paragraph}};
use syntect::{parsing::SyntaxSet, highlighting::ThemeSet, easy::HighlightLines};

use super::{syntect_tui::into_span, components::action_text_areas::TextArea};

pub fn spaces(size: u8) -> &'static str {
    const SPACES: &str = "                                                                                                                                                                                                                                                                ";
    &SPACES[..size as usize]
}

pub fn num_digits(i: usize) -> u8 {
    f64::log10(i as f64) as u8 + 1
}

#[derive(Default, Debug)]
pub struct Viewport(AtomicU64);

impl Clone for Viewport {
    fn clone(&self) -> Self {
        let u = self.0.load(Ordering::Relaxed);
        Viewport(AtomicU64::new(u))
    }
}

impl Viewport {
    pub fn scroll_top(&self) -> (u16, u16) {
        let u = self.0.load(Ordering::Relaxed);
        ((u >> 16) as u16, u as u16)
    }

    pub fn rect(&self) -> (u16, u16, u16, u16) {
        let u = self.0.load(Ordering::Relaxed);
        let width = (u >> 48) as u16;
        let height = (u >> 32) as u16;
        let row = (u >> 16) as u16;
        let col = u as u16;
        (row, col, width, height)
    }

    pub fn position(&self) -> (u16, u16, u16, u16) {
        let (row_top, col_top, width, height) = self.rect();
        let row_bottom = row_top.saturating_add(height).saturating_sub(1);
        let col_bottom = col_top.saturating_add(width).saturating_sub(1);

        (
            row_top,
            col_top,
            cmp::max(row_top, row_bottom),
            cmp::max(col_top, col_bottom),
        )
    }

    fn store(&self, row: u16, col: u16, width: u16, height: u16) {
        // Pack four u16 values into one u64 value
        let u =
            ((width as u64) << 48) | ((height as u64) << 32) | ((row as u64) << 16) | col as u64;
        self.0.store(u, Ordering::Relaxed);
    }

    pub fn scroll(&mut self, rows: i16, cols: i16) {
        fn apply_scroll(pos: u16, delta: i16) -> u16 {
            if delta >= 0 {
                pos.saturating_add(delta as u16)
            } else {
                pos.saturating_sub(-delta as u16)
            }
        }

        let u = self.0.get_mut();
        let row = apply_scroll((*u >> 16) as u16, rows);
        let col = apply_scroll(*u as u16, cols);
        *u = (*u & 0xffff_ffff_0000_0000) | ((row as u64) << 16) | (col as u64);
    }
}

pub struct Renderer<'a>{
    pub(crate) text_area: &'a TextArea<'a>,
    pub(crate) viewport: &'a Viewport,
}

impl<'a> Renderer<'a> {
    #[inline]
    fn text(&'a self, top_row: usize, height: usize) -> Text<'a> {
        let lines_len = self.text_area.get_text_area().lines().len();
        let lnum_len = f64::log10(lines_len as f64) as u8 + 1;
        let bottom_row = cmp::min(top_row + height, lines_len);
        let mut lines = Vec::with_capacity(bottom_row - top_row);

        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ps.find_syntax_by_extension("json").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
        let mut i = top_row;
        for line in &self.text_area.get_text_area().lines()[top_row..bottom_row] {
            // not cursor line
            if i != self.text_area.get_text_area().cursor().0 as usize {
                let mut spans = h.highlight_line(line, &ps)
                .unwrap().into_iter()
                .filter_map(|segment| into_span(segment).ok())
                .collect::<Vec<_>>();
                let pad = spaces(lnum_len -  num_digits(i + 1) + 1);
                spans.insert(0, Span::styled(format!("{}{} ", pad, i + 1), self.text_area.get_text_area().line_number_style().unwrap_or_else(|| Style::default())));
                lines.push(Line::from(spans));
                i += 1;
                continue;
            }
            // cursor line
            let mut spans = vec![];
             // Add line number
            let pad = spaces(lnum_len -  num_digits(i + 1) + 1);
            spans.insert(0, Span::styled(format!("{}{} ", pad, i + 1), self.text_area.get_text_area().line_number_style().unwrap_or_else(|| Style::default())));
            let mut char_count = 0;
            let mut cursor_set = false;
            for (style, v) in h.highlight_line(line, &ps).unwrap().into_iter() {
                if v.is_empty() {
                    continue;
                }
                let current_char_count = char_count.clone();
                char_count += v.chars().count();
                // do not need to split the current span
                if char_count < self.text_area.get_text_area().cursor().1  || cursor_set {
                    spans.push(into_span((style, v)).unwrap());
                    continue;
                 } else {
                    //println!("cursor: {},  current_char_count: {}", cursor.1, current_char_count);

                    let cursor_pos = if current_char_count <= self.text_area.get_text_area().cursor().1 {
                        self.text_area.get_text_area().cursor().1 - current_char_count
                    } else {
                        0
                    };
                    spans.push(into_span((style, v.get(0..cursor_pos).unwrap_or_else(|| &v))).unwrap());
                    if cursor_pos < v.len() {
                        spans.push(Span::styled(v.get(cursor_pos..cursor_pos + 1).unwrap_or_else(|| &v), Style::default().add_modifier(Modifier::REVERSED)));
                        spans.push(into_span((style, &v[cursor_pos + 1..])).unwrap());
                        cursor_set = true;
                    }
                }
            }
            if !cursor_set {
                spans.push(Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)));
            }
            lines.push(Line::from(spans));
            i += 1;
        }

        Text::from(lines)
    }
}

impl<'a> Widget for Renderer<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let ta = self.text_area.get_text_area();
        let Rect { width, height, .. } = if let Some(b) = ta.block() {
            b.inner(area)
        } else {
            area
        };

        fn next_scroll_top(prev_top: u16, cursor: u16, length: u16) -> u16 {
            if cursor < prev_top {
                cursor
            } else if prev_top + length <= cursor {
                cursor + 1 - length
            } else {
                prev_top
            }
        }

        let cursor = ta.cursor();
        let (top_row, top_col) = self.viewport.scroll_top();
        let top_row = next_scroll_top(top_row, cursor.0 as u16, height);
        let top_col = next_scroll_top(top_col, cursor.1 as u16, width);

        let (text, style) = (self.text(top_row as usize, height as usize), ta.style());

        // To get fine control over the text color and the surrrounding block they have to be rendered separately
        // see https://github.com/ratatui-org/ratatui/issues/144
        let mut text_area = area;
        let mut inner = Paragraph::new(text)
            .style(style)
            .alignment(ta.alignment());
        if let Some(b) = ta.block() {
            text_area = b.inner(area);
            b.clone().render(area, buf)
        }
        if top_col != 0 {
            inner = inner.scroll((0, top_col));
        }

        // Store scroll top position for rendering on the next tick
        self.viewport.store(top_row, top_col, width, height);

        inner.render(text_area, buf);
    }
}
