use std::io;

use super::{app::ActiveArea, components::run_action::ActiveRunActionArea};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{block::Title, Block, Borders, ListState, Tabs},
    Frame,
};

pub trait Component {
    fn render(
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

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

//
pub fn render_tabs<'a, T>(
    tabs_title: Vec<&'static str>,
    titles: Vec<T>,
    current_active_area: &ActiveRunActionArea,
    target_area: &ActiveRunActionArea,
    selected_tab: usize,
) -> Tabs<'a>
where
    T: Into<Title<'a>>,
    Line<'a>: From<Vec<T>>,
{
    Tabs::new(tabs_title)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(titles)
                .title_alignment(Alignment::Right)
                .border_style(Style::default().fg(if current_active_area == target_area {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .divider(Span::raw("|"))
        .select(selected_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Yellow).bold())
}
