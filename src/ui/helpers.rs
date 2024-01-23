use std::{cmp, io};

use super::{app::ActiveArea, syntect_tui::into_span};
use ratatui::{
    backend::Backend,
    buffer::Buffer,
    layout::Rect,
    style::Color,
    text::{Line, Text},
    widgets::{ListState, Paragraph, Widget},
    Frame,
};
use serde_json::Value;
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};

pub trait Component {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        current_active_area: ActiveArea,
    ) -> io::Result<()>;
}

pub trait Selectable {
    fn selected(&self) -> Option<usize>;
    fn select(&mut self, index: Option<usize>);
}

impl Selectable for ListState {
    fn selected(&self) -> Option<usize> {
        self.selected()
    }

    fn select(&mut self, index: Option<usize>) {
        self.select(index)
    }
}

pub trait Stateful<T>
where
    T: Selectable,
{
    fn items_len(&self) -> usize;
    fn state(&mut self) -> &mut T;
    fn next(&mut self) {
        let items_len = self.items_len();
        let i = match self.state().selected() {
            Some(i) => {
                if i >= items_len - 1 {
                    items_len - 1
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state().select(Some(i));
    }
    fn previous(&mut self) {
        let i = match self.state().selected() {
            Some(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state().select(Some(i));
    }

    fn unselect(&mut self) {
        self.state().select(None);
    }
}

#[derive(Default, Clone)]
pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> Stateful<ListState> for StatefulList<T> {
    fn items_len(&self) -> usize {
        self.items.len()
    }

    fn state(&mut self) -> &mut ListState {
        &mut self.state
    }
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }
}

pub fn highlight_if_needed(
    current_active_area: ActiveArea,
    target_active_area: ActiveArea,
) -> Color {
    if current_active_area == target_active_area {
        Color::Green
    } else {
        Color::DarkGray
    }
}

pub fn payload_as_str_pretty(payload: Option<&Value>) -> anyhow::Result<String> {
    let r = serde_json::to_string_pretty(
        &payload
            .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null),
    )?;
    Ok(r)
}
