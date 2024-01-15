use crate::commands::run::_run_helper::ANONYMOUS_ACTION;
use crate::db::db_trait::Db;
use crate::db::dto::{Action, Project};
use crate::ui::helpers::{Stateful, StatefulList};
use crate::ui::run_ui::UIRunner;
use crate::utils::human_readable_date;
use crossterm::event::{self};
use ratatui::backend::Backend;
use ratatui::Frame;
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};
use serde_json::Value;
use std::collections::HashMap;
use std::{io, thread};
use tokio::runtime::Handle;
use tui_textarea::{Input, Key};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum ActiveArea {
    #[default]
    ProjectPane,
    ActionPane,
    BodyExample,
    ResponseExample,
}

#[derive(Clone)]
pub(crate) struct ProjectUI<'a> {
    db: Box<dyn Db>,
    active_area: ActiveArea,
    projects: StatefulList<Project>,
    actions: StatefulList<Action>,
    body_ex_text_area: Option<tui_textarea::TextArea<'a>>,
    resp_ex_text_area: Option<tui_textarea::TextArea<'a>>,
    current_action_index: (String, bool, bool),
}

impl<'a> ProjectUI<'a> {
    pub fn new(projects: Vec<Project>, db_handler: Box<dyn Db>) -> Self {
        Self {
            db: db_handler,
            active_area: ActiveArea::ProjectPane,
            projects: StatefulList::with_items(projects),
            actions: StatefulList {
                state: ListState::default(),
                items: Vec::new(),
            },
            body_ex_text_area: None,
            resp_ex_text_area: None,
            current_action_index: ("".to_string(), false, false),
        }
    }

    // clearing a text area
    fn clear_text_area(text_area: &mut tui_textarea::TextArea) {
        text_area.move_cursor(tui_textarea::CursorMove::Bottom);
        for _ in 0..1000 {
            text_area.delete_newline();
            text_area.delete_str(0, 100000);
        }
    }

    // set text of a text area
    fn set_text(text_area: &mut tui_textarea::TextArea, text: &str) {
        text.lines().for_each(|l| {
            text_area.insert_str(l);
            text_area.insert_newline()
        });
        text_area.move_cursor(tui_textarea::CursorMove::Top)
    }

    fn payload_as_str_pretty(payload: Option<&Value>) -> anyhow::Result<String> {
        let r = serde_json::to_string_pretty(
            &payload
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null),
        )?;
        Ok(r)
    }

    fn update_actions(&mut self) {
        let handle = Handle::current();
        let self_cloned = self.clone();

        let actions = thread::spawn(move || {
            handle.block_on(async move {
                let selected_item = self_cloned.projects.items
                    [self_cloned.projects.state.selected().unwrap()]
                .clone();
                self_cloned
                    .db
                    .get_actions(Some(&selected_item.name))
                    .await
                    .unwrap()
            })
        })
        .join()
        .unwrap();

        self.actions = StatefulList::with_items(actions);
    }

    fn set_current_action_index(&mut self) {
        self.current_action_index = (
            self.actions
                .items
                .get(self.actions.state.selected().unwrap())
                .unwrap()
                .name
                .as_ref()
                .unwrap_or(&"UNKNOWN".to_string())
                .clone(),
            false,
            false,
        );
    }

    fn get_color(&self, area: ActiveArea) -> Color {
        if area == self.active_area {
            Color::Green
        } else {
            Color::DarkGray
        }
    }
    fn build_ui<B: Backend>(&mut self, frame: &mut Frame<B>) -> io::Result<()> {
        let all = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Percentage(98),
                Percentage(2), // examples
            ])
            .split(frame.size());

        // render status bar displaying shortcuts
        let status_bar = Paragraph::new(
            Line::from(

                vec![
                    if self.active_area == ActiveArea::ProjectPane {
                        Span::styled(" → action pane   ↓ next project  ↑ previous project", Style::default().fg(Color::DarkGray))
                    } else if self.active_area == ActiveArea::ActionPane {
                        Span::styled(" ← project pane   ↓ next action  ↑ previous action    Ctrl-b body example    Ctrl-r response example    ESC action pane", Style::default().fg(Color::DarkGray))
                    } else {
                        Span::styled("", Style::default().fg(Color::DarkGray))
                    },
                ]
            )
        ).bg(Color::DarkGray).fg(Color::White);
        frame.render_widget(status_bar, all[1]);

        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Percentage(30), Percentage(70), Min(0)])
            .split(all[0]);

        let project_list = List::new(
            self.projects
                .items
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let mut conf_keys = p
                        .get_project_conf()
                        .unwrap_or(HashMap::new())
                        .keys()
                        .map(String::to_string)
                        .collect::<Vec<_>>();
                    conf_keys.sort();
                    ListItem::new(vec![Line::styled(
                        format!(" {}. {}({})", i, p.name.clone(), conf_keys.join(", ")),
                        Style::default().fg(Color::LightGreen).bold(),
                    )])
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
        .highlight_style(Style::default().fg(Color::Yellow))
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
                    let r = &a.actions[0];
                    let v = &r.verb;
                    let url = r.urls.iter().next().cloned().unwrap_or_default();
                    let action_name = a.name.as_deref().unwrap_or(ANONYMOUS_ACTION);
                    ListItem::new(vec![
                        Line::styled(action_name, Style::default().fg(Color::LightGreen).bold()),
                        Line::from(vec![
                            Span::raw("    "),
                            match v.as_str() {
                                "POST" => Span::styled(
                                    v,
                                    Style::default().fg(Color::DarkGray).bg(Color::Green),
                                ),
                                "GET" => Span::styled(
                                    v,
                                    Style::default().fg(Color::DarkGray).bg(Color::Blue),
                                ),
                                "DELETE" => Span::styled(
                                    v,
                                    Style::default().fg(Color::DarkGray).bg(Color::Red),
                                ),
                                "PUT" => Span::styled(
                                    v,
                                    Style::default()
                                        .fg(Color::DarkGray)
                                        // purple
                                        .bg(Color::Rgb(128, 0, 128)),
                                ),
                                "PATCH" => Span::styled(
                                    v,
                                    // purple
                                    Style::default()
                                        .fg(Color::DarkGray)
                                        .bg(Color::Rgb(255, 128, 0)),
                                ),
                                _ => Span::styled(
                                    v,
                                    Style::default().fg(Color::DarkGray).bg(Color::Yellow),
                                ),
                            },
                            Span::raw(" "),
                            Span::styled(url, Style::default().fg(Color::LightBlue)),
                            Span::raw(" "),
                            Span::styled(
                                if r.body.1 {
                                    "(form)"
                                } else if r.body.2 {
                                    "(url encoded)"
                                } else {
                                    "(json)"
                                },
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                a.created_at
                                    .as_ref()
                                    .map(human_readable_date)
                                    .unwrap_or("".to_string()),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                a.updated_at
                                    .as_ref()
                                    .map(human_readable_date)
                                    .unwrap_or("".to_string()),
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
        // if not select action skip the rendering
        if selected_action_index.is_none() {
            return Ok(());
        }

        // getting action body and response example
        let current_action = self.actions.items[selected_action_index.unwrap()].clone();
        let body_example = Self::payload_as_str_pretty(current_action.body_example.as_ref())
            .unwrap_or("FAILED TO PARSE BODY EXAMPLE".to_string());
        let response_example =
            Self::payload_as_str_pretty(current_action.response_example.as_ref())
                .unwrap_or("FAILED TO PARSE RESPONSE EXAMPLE".to_string());

        vec![
            self.body_ex_text_area.as_mut(),
            self.resp_ex_text_area.as_mut(),
        ]
        .iter_mut()
        .zip(vec![ActiveArea::BodyExample, ActiveArea::ResponseExample].iter())
        .for_each(|(text_area, area)| {
            let is_body_example = area == &ActiveArea::BodyExample;
            if let Some(t) = text_area.as_mut() {
                t.set_block(t.block().unwrap().clone().border_style(Style::default().fg(
                    if area == &self.active_area {
                        Color::Green
                    } else {
                        Color::DarkGray
                    },
                )));
                if current_action
                    .name
                    .as_ref()
                    .unwrap_or(&"UNKNOWN".to_string())
                    == &self.current_action_index.0
                {
                    let to_setup = if is_body_example {
                        !self.current_action_index.1
                    } else {
                        !self.current_action_index.2
                    };
                    if to_setup {
                        Self::clear_text_area(t);
                        Self::set_text(
                            t,
                            if is_body_example {
                                &body_example
                            } else {
                                &response_example
                            },
                        );
                        if is_body_example {
                            self.current_action_index.1 = true;
                        } else {
                            self.current_action_index.2 = true;
                        }
                    }
                }
            };
            frame.render_widget(
                text_area.as_mut().unwrap().widget(),
                bottom_layout[if is_body_example { 0 } else { 1 }],
            );
        });

        Ok(())
    }
}

impl UIRunner for ProjectUI<'_> {
    fn init(&mut self) {
        let mut body_ex_text_area = tui_textarea::TextArea::default();
        body_ex_text_area.set_line_number_style(Style::default().bg(Color::DarkGray));
        body_ex_text_area.set_block(
            Block::default()
                .title("Body example".gray())
                .style(Style::reset())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.get_color(ActiveArea::ResponseExample))),
        );
        self.body_ex_text_area = Some(body_ex_text_area);

        let mut resp_ex_text_area = tui_textarea::TextArea::default();
        resp_ex_text_area.set_line_number_style(Style::default().bg(Color::DarkGray));
        resp_ex_text_area.set_block(
            Block::default()
                .title("Response example".gray())
                .style(Style::reset())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.get_color(ActiveArea::ResponseExample))),
        );
        self.resp_ex_text_area = Some(resp_ex_text_area);
    }

    fn handle_event(&mut self) -> io::Result<bool> {
        let ev = event::read()?;
        match ev.into() {
            Input { key: Key::Esc, .. } => match self.active_area {
                ActiveArea::BodyExample | ActiveArea::ResponseExample => {
                    self.active_area = ActiveArea::ActionPane;
                }
                // early escape
                _ => return Ok(true),
            },
            input @ Input {
                key: Key::Right,
                ctrl: false,
                alt: false,
            } => match &mut self.active_area {
                ActiveArea::ProjectPane => {
                    self.active_area = ActiveArea::ActionPane;
                }
                ActiveArea::BodyExample => {
                    let _ = self.body_ex_text_area.as_mut().unwrap().input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self.resp_ex_text_area.as_mut().unwrap().input(input);
                }
                _ => {}
            },
            input @ Input {
                key: Key::Left,
                ctrl: false,
                alt: false,
            } => match &mut self.active_area {
                ActiveArea::ActionPane => {
                    self.active_area = ActiveArea::ProjectPane;
                }
                ActiveArea::BodyExample => {
                    let _ = self.body_ex_text_area.as_mut().unwrap().input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self.resp_ex_text_area.as_mut().unwrap().input(input);
                }
                _ => {}
            },
            input @ Input {
                key: Key::Up,
                ctrl: false,
                alt: false,
            } => match &mut self.active_area {
                ActiveArea::ProjectPane => {
                    self.projects.previous();
                    self.update_actions();
                }
                ActiveArea::ActionPane => {
                    self.actions.previous();
                    self.set_current_action_index();
                }
                ActiveArea::BodyExample => {
                    let _ = self.body_ex_text_area.as_mut().unwrap().input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self.resp_ex_text_area.as_mut().unwrap().input(input);
                }
            },
            input @ Input {
                key: Key::Down,
                ctrl: false,
                alt: false,
            } => match self.active_area {
                ActiveArea::ProjectPane => {
                    self.projects.next();
                    self.update_actions();
                }
                ActiveArea::ActionPane => {
                    self.actions.next();
                    self.set_current_action_index();
                }
                ActiveArea::BodyExample => {
                    let _ = self.body_ex_text_area.as_mut().unwrap().input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self.resp_ex_text_area.as_mut().unwrap().input(input);
                }
            },
            input => match self.active_area {
                ActiveArea::ResponseExample => {
                    let _ = self.resp_ex_text_area.as_mut().unwrap().input(input);
                }
                ActiveArea::BodyExample => {
                    let _ = self.body_ex_text_area.as_mut().unwrap().input(input);
                }
                ActiveArea::ActionPane => match input {
                    Input {
                        key: Key::Char('r'),
                        ctrl: true,
                        ..
                    } => self.active_area = ActiveArea::ResponseExample,
                    Input {
                        key: Key::Char('b'),
                        ctrl: true,
                        ..
                    } => self.active_area = ActiveArea::BodyExample,
                    Input {
                        key: Key::Char('q'),
                        ctrl: false,
                        alt: false,
                    } => return Ok(true),
                    _ => {}
                },
                _ => {
                    if let Input {
                        key: Key::Char('q'),
                        ..
                    } = input
                    {
                        return Ok(true);
                    }
                }
            },
        }

        Ok(false)
    }

    fn ui<B: Backend>(&mut self, f: &mut Frame<B>) {
        let r = self.build_ui(f);
        if let Err(e) = r {
            println!("Error: {}", e);
        }
    }
}
