use std::{
    cmp,
    sync::atomic::{AtomicU64, Ordering},
};

use lazy_static::lazy_static;
use ratatui::{
    prelude::*,
    widgets::{Paragraph, Widget},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use super::{components::action_text_areas::TextArea, syntect_tui::into_span};

#[inline]
pub fn spaces(size: u8) -> &'static str {
    const SPACES: &str = "                                                                                                                                                                                                                                                                ";
    &SPACES[..size as usize]
}

#[inline]
pub fn num_digits(i: usize) -> u8 {
    f64::log10(i as f64) as u8 + 1
}

lazy_static! {
    pub static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
    pub static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
    pub static ref TOML_SYNTAX: SyntaxReference = SYNTAX_SET
        .find_syntax_by_extension("toml")
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
        .clone();
    pub static ref JSON_SYNTAX: SyntaxReference = SYNTAX_SET
        .find_syntax_by_extension("json")
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
        .clone();
    pub static ref THEME: Theme = THEME_SET.themes["base16-ocean.dark"].clone();
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

    fn store(&self, row: u16, col: u16, width: u16, height: u16) {
        // Pack four u16 values into one u64 value
        let u =
            ((width as u64) << 48) | ((height as u64) << 32) | ((row as u64) << 16) | col as u64;
        self.0.store(u, Ordering::Relaxed);
    }
}

pub struct Renderer<'a> {
    pub(crate) text_area: &'a TextArea<'a>,
    pub(crate) viewport: &'a Viewport,
    pub(crate) extension: &'a str,
}

impl<'a> Renderer<'a> {
    #[inline]
    fn text(&'a self, top_row: usize, height: usize) -> Text<'a> {
        let lines_len = self.text_area.get_text_area().lines().len();
        let lnum_len = f64::log10(lines_len as f64) as u8 + 1;
        let bottom_row = cmp::min(top_row + height, lines_len);
        let mut lines = Vec::with_capacity(bottom_row - top_row);

        let syntax = match self.extension {
            "json" => &*JSON_SYNTAX,
            "toml" => &*TOML_SYNTAX,
            _ => SYNTAX_SET.find_syntax_plain_text(),
        };

        let mut h = HighlightLines::new(syntax, &THEME);
        let mut i = top_row;

        let text_area = self.text_area.get_text_area();

        for line in &text_area.lines()[top_row..bottom_row] {
            // not cursor line
            if i != text_area.cursor().0 {
                let mut spans = h
                    .highlight_line(line, &SYNTAX_SET)
                    .unwrap()
                    .into_iter()
                    .filter_map(|segment| into_span(segment).ok())
                    .collect::<Vec<_>>();
                let pad = spaces(lnum_len - num_digits(i + 1) + 1);
                spans.insert(
                    0,
                    Span::styled(
                        format!("{}{} ", pad, i + 1),
                        text_area.line_number_style().unwrap_or_default(),
                    ),
                );
                lines.push(Line::from(spans));
                i += 1;
                continue;
            }
            // cursor line
            let mut spans = vec![];
            // Add line number
            let pad = spaces(lnum_len - num_digits(i + 1) + 1);
            spans.insert(
                0,
                Span::styled(
                    format!("{}{} ", pad, i + 1),
                    text_area.line_number_style().unwrap_or_default(),
                ),
            );
            let mut char_count = 0;
            let mut cursor_set = false;
            for (mut style, v) in h.highlight_line(line, &SYNTAX_SET).unwrap().into_iter() {
                if v.is_empty() {
                    continue;
                }
                // set font style to underline
                style.font_style = FontStyle::UNDERLINE;

                let current_char_count = char_count;
                char_count += v.chars().count();
                // do not need to split the current span
                if char_count < text_area.cursor().1 || cursor_set {
                    spans.push(into_span((style, v)).unwrap());
                    continue;
                }
                let cursor_pos = if current_char_count <= text_area.cursor().1 {
                    text_area.cursor().1 - current_char_count
                } else {
                    0
                };
                spans.push(into_span((style, v.get(0..cursor_pos).unwrap_or(v))).unwrap());
                if cursor_pos < v.len() {
                    spans.push(Span::styled(
                        v.get(cursor_pos..cursor_pos + 1).unwrap_or(v),
                        Style::default().add_modifier(Modifier::REVERSED),
                    ));
                    spans.push(into_span((style, &v[cursor_pos + 1..])).unwrap());
                    cursor_set = true;
                }
            }
            if !cursor_set {
                spans.push(Span::styled(
                    " ",
                    Style::default().add_modifier(Modifier::REVERSED),
                ));
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
        let mut inner = Paragraph::new(text).style(style).alignment(ta.alignment());
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
