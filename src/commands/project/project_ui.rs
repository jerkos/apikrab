use crate::db::db_handler::DBHandler;
use crate::db::dto::{Action, Project};
use crate::ui::helpers::{Stateful, StatefulList};
use crate::ui::run_ui::UIRunner;
use crate::utils::random_emoji;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::Backend;
use ratatui::Frame;
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};
use std::{io, thread};
use tokio::runtime::Handle;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ActiveArea {
    ProjectPane,
    ActionPane,
    BodyExample,
    ResponseExample,
}

impl Default for ActiveArea {
    fn default() -> Self {
        ActiveArea::ProjectPane
    }
}

#[derive(Clone)]
pub(crate) struct ProjectUI {
    db: DBHandler,
    active_area: ActiveArea,
    projects: StatefulList<Project>,
    actions: StatefulList<Action>,
    body_example_scroll: u16,
    response_example_scroll: u16,
}

impl ProjectUI {
    pub fn new(projects: Vec<Project>, db_handler: DBHandler) -> Self {
        Self {
            db: db_handler,
            active_area: ActiveArea::ProjectPane,
            projects: StatefulList::with_items(projects),
            actions: StatefulList {
                state: ListState::default(),
                items: Vec::new(),
            },
            body_example_scroll: 0,
            response_example_scroll: 0,
        }
    }

    fn update_actions(&mut self) {
        let handle = Handle::current();
        let self_cloned = self.clone();

        let actions = thread::spawn(move || {
            let v = handle.block_on(async move {
                let selected_item = self_cloned.projects.items
                    [self_cloned.projects.state.selected().unwrap()]
                .clone();
                self_cloned
                    .db
                    .get_actions(&selected_item.name)
                    .await
                    .unwrap()
            });
            v
        })
        .join()
        .unwrap();

        self.actions = StatefulList::with_items(actions);
    }

    fn get_color(&self, area: ActiveArea) -> Color {
        if area == self.active_area {
            Color::Blue
        } else {
            Color::DarkGray
        }
    }
    fn build_ui<B: Backend>(&mut self, frame: &mut Frame<B>) {
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Percentage(50),
                Percentage(50), // examples
                Min(0),         // fills remaining space
            ])
            .split(frame.size());

        let project_list = List::new(
            self.projects
                .items
                .iter()
                .map(|p| {
                    let mut conf_keys = p
                        .get_conf()
                        .keys()
                        .map(String::to_string)
                        .collect::<Vec<_>>();
                    conf_keys.sort();
                    ListItem::new(vec![
                        Line::styled(
                            format!(
                                " {} {}({})",
                                random_emoji(),
                                p.name.clone(),
                                conf_keys.join(", ")
                            ),
                            Style::default().fg(Color::LightGreen).bold(),
                        ),
                        Line::styled(
                            format!(
                                "    {} | {}",
                                p.test_url.as_ref().unwrap_or(&"None".to_string()),
                                p.prod_url.as_ref().unwrap_or(&"None".to_string()),
                            ),
                            Style::default().fg(Color::LightBlue),
                        ),
                    ])
                })
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title("Projects".gray())
                .style(Style::reset())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.get_color(ActiveArea::ProjectPane))),
        )
        .highlight_style(Style::default().fg(Color::White))
        .highlight_symbol(">>");

        // rendering
        frame.render_stateful_widget(project_list, main_layout[0], &mut self.projects.state);

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Percentage(50),
                Percentage(50), // examples
            ])
            .split(main_layout[1]);

        let action_list = List::new(
            self.actions
                .items
                .iter()
                .map(|a| {
                    ListItem::new(vec![
                        Line::styled(
                            a.name.clone(),
                            Style::default().fg(Color::LightGreen).bold(),
                        ),
                        Line::from(vec![
                            Span::raw("    "),
                            match a.verb.as_str() {
                                "POST" => Span::styled(
                                    a.verb.clone(),
                                    Style::default().fg(Color::DarkGray).bg(Color::Green),
                                ),
                                "GET" => Span::styled(
                                    a.verb.clone(),
                                    Style::default().fg(Color::DarkGray).bg(Color::Blue),
                                ),
                                _ => Span::styled(
                                    a.verb.clone(),
                                    Style::default().fg(Color::DarkGray).bg(Color::Red),
                                ),
                            },
                            Span::raw(" "),
                            Span::styled(a.url.clone(), Style::default().fg(Color::LightBlue)),
                            Span::raw(" "),
                            Span::styled(
                                if a.is_form() { "(form)" } else { "(json)" },
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]),
                    ])
                })
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title("Actions".gray())
                .style(Style::reset())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.get_color(ActiveArea::ActionPane))),
        )
        .highlight_style(Style::default().fg(Color::White))
        .highlight_symbol(">>");

        frame.render_stateful_widget(action_list, right_layout[0], &mut self.actions.state);

        let bottom_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Percentage(50),
                Percentage(50), // examples
            ])
            .split(right_layout[1]);

        let selected_action_index = self.actions.state.selected();
        if selected_action_index.is_none() {
            return;
        }
        let current_action = &self.actions.items[selected_action_index.unwrap()];
        let action_body = serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(
                current_action
                    .body_example
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or(&"{}"),
            )
            .unwrap(),
        )
        .unwrap();

        let response_example = serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(
                current_action
                    .response_example
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or(&"{}"),
            )
            .unwrap(),
        )
        .unwrap();

        let action_info = Paragraph::new(Text::from(action_body))
            .block(
                Block::default()
                    .title("body example".gray())
                    .style(Style::reset())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.get_color(ActiveArea::BodyExample))),
            )
            .wrap(Wrap { trim: true })
            .scroll((self.body_example_scroll, 0));

        frame.render_widget(action_info, bottom_layout[0]);

        let response_info = Paragraph::new(Text::from(response_example))
            .block(
                Block::default()
                    .title("Response example".gray())
                    .style(Style::reset())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.get_color(ActiveArea::ResponseExample))),
            )
            .wrap(Wrap { trim: true })
            .scroll((self.response_example_scroll, 0));

        frame.render_widget(response_info, bottom_layout[1]);
    }
}

impl UIRunner for ProjectUI {
    fn handle_event(&mut self) -> io::Result<bool> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Char('b') => {
                        self.active_area = ActiveArea::BodyExample;
                    }
                    KeyCode::Char('r') => {
                        self.active_area = ActiveArea::ResponseExample;
                    }
                    KeyCode::Esc => {
                        if self.active_area == ActiveArea::BodyExample
                            || self.active_area == ActiveArea::ResponseExample
                        {
                            self.active_area = ActiveArea::ActionPane;
                        }
                    }
                    KeyCode::Left => {
                        self.active_area = ActiveArea::ProjectPane;
                    }
                    KeyCode::Right => {
                        self.active_area = ActiveArea::ActionPane;
                    }
                    KeyCode::Up => match &mut self.active_area {
                        ActiveArea::ProjectPane => {
                            self.projects.previous();
                            self.update_actions();
                        }
                        ActiveArea::ActionPane => {
                            self.actions.previous();
                            self.response_example_scroll = 0;
                            self.body_example_scroll = 0;
                        }
                        ActiveArea::BodyExample => {
                            self.body_example_scroll = if self.body_example_scroll == 0 {
                                0
                            } else {
                                self.body_example_scroll - 1
                            }
                        }
                        ActiveArea::ResponseExample => {
                            self.response_example_scroll = if self.response_example_scroll == 0 {
                                0
                            } else {
                                self.response_example_scroll - 1
                            }
                        }
                    },
                    KeyCode::Down => match self.active_area {
                        ActiveArea::ProjectPane => {
                            self.projects.next();
                            self.update_actions();
                        }
                        ActiveArea::ActionPane => {
                            self.actions.next();
                            self.response_example_scroll = 0;
                            self.body_example_scroll = 0;
                        }
                        ActiveArea::BodyExample => self.body_example_scroll += 1,
                        ActiveArea::ResponseExample => self.response_example_scroll += 1,
                    },
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    fn ui<B: Backend>(&mut self, f: &mut Frame<B>) {
        self.build_ui(f);
    }
}
