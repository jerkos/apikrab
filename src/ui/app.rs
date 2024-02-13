use crate::commands::run::_printer::Printer;
use crate::commands::run::_progress_bar::init_progress_bars;
use crate::commands::run::_test_checker::UnaryTestResult;
use crate::commands::run::action::R;
use crate::db::db_trait::Db;
use crate::db::dto::{Action, Project};
use crate::domain::DomainAction;
use crate::http::Api;
use crate::ui::helpers::{Stateful, StatefulList};
use crate::ui::run_ui::UIRunner;
use crossterm::event::{self};
use indicatif::ProgressDrawTarget;
use ratatui::backend::Backend;
use ratatui::Frame;
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use std::{io, vec};
use tokio::sync::mpsc;
use tui_textarea::{Input, Key};

use super::components::action_list::ActionList;
use super::components::action_text_areas::{
    text_area, ActionTextAreas, DisplayFromAction, Examples,
};
use super::components::project_list::ProjectList;
use super::components::run_action::{ActiveTextArea, RunAction, RunStatus, TestStatus};
use super::components::status_bar::status_bar;
use super::custom_renderer;
use super::helpers::Component;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActiveArea {
    #[default]
    ProjectPane,
    ActionPane,
    BodyExample,
    ResponseExample,
    // entire screen
    RunAction,
}

#[derive(Debug)]
pub enum Message {
    RunResult(Vec<(Vec<R>, Vec<Vec<UnaryTestResult>>)>),
    UpdateAction(Vec<Action>),
}

lazy_static! {
    pub static ref EXAMPLE: Box<dyn DisplayFromAction> = Box::new(Examples {});
}

pub(crate) struct App<'a> {
    db: Box<dyn Db>,
    active_area: ActiveArea,
    // all projects
    projects: StatefulList<Project>,
    // current displayed actions
    actions: StatefulList<Action>,
    action_text_areas: ActionTextAreas<'a>,
    run_action_pane: RunAction<'a>,
    current_action: Option<Action>,
    action_has_changed: bool,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}

impl<'a> App<'a> {
    pub fn new(projects: Vec<Project>, db_handler: Box<dyn Db>) -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            db: db_handler,
            active_area: ActiveArea::ProjectPane,
            projects: StatefulList::with_items(projects),
            actions: StatefulList {
                state: ListState::default(),
                items: Vec::new(),
            },
            action_text_areas: ActionTextAreas::new("Body", "Response", &EXAMPLE),
            run_action_pane: RunAction {
                active_text_area: Default::default(),
                edit_text_area: text_area("Edit"),
                edit_text_area_viewport: custom_renderer::Viewport::default(),
                response_body_text_area: text_area("Response body"),
                response_body_text_area_viewport: custom_renderer::Viewport::default(),
                response_headers_text_area: text_area("Headers"),
                response_headers_text_area_viewport: custom_renderer::Viewport::default(),
                action_name: None,
                project_name: None,
                status: RunStatus::Idle,
                test_status: TestStatus::NotRun,
                test_results: None,
                fetch_result: None,
            },
            current_action: None,
            action_has_changed: false,
            tx,
            rx,
        }
    }

    /// Update the actions list
    /// This function will spawn a new tokio task to fetch the actions
    fn update_actions(&mut self) {
        let projects = self.projects.clone();
        let db = self.db.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let selected_item = projects.items[projects.state.selected().unwrap()].clone();
            let actions = db.get_actions(Some(&selected_item.name)).await.unwrap();
            tx.send(Message::UpdateAction(actions)).await.unwrap();
        });
    }

    /// Run the current action
    /// This function will spawn a new tokio task to run the action
    fn run_action(&mut self) {
        let edit_content = self
            .run_action_pane
            .edit_text_area
            .get_text_area()
            .lines()
            .join("\n");
        let actions = serde_json::from_str::<Vec<DomainAction>>(&edit_content);
        if actions.is_err() {
            return;
        }
        let actions = actions.unwrap();

        let db = self.db.clone();
        let api = Api::new(Some(5), false);
        let mut ctx = HashMap::new();
        let (multi, pb) = init_progress_bars(actions.len() as u64);
        multi.set_draw_target(ProgressDrawTarget::hidden());
        pb.set_draw_target(ProgressDrawTarget::hidden());
        let mut printer = Printer::new(true, false, false);
        let tx = self.tx.clone();

        // updating run action pane
        self.run_action_pane.status = RunStatus::Running;

        tokio::spawn(async move {
            let mut results: Vec<(Vec<R>, Vec<Vec<UnaryTestResult>>)> = vec![];
            for action in actions {
                let r = action
                    .run_with_tests(None, &mut ctx, &*db, &api, &mut printer, &multi, &pb)
                    .await;
                results.push(r);
            }
            tx.send(Message::RunResult(results)).await.unwrap();
        });
    }

    fn set_current_action(&mut self) {
        self.current_action =
            Some(self.actions.items[self.actions.state.selected().unwrap()].clone());
        self.action_has_changed = true;
    }

    fn build_ui<B: Backend>(&mut self, frame: &mut Frame) -> io::Result<()> {
        let all = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Percentage(98),
                Percentage(2), // examples
            ])
            .split(frame.size());

        if self.active_area == ActiveArea::RunAction {
            if self.current_action.is_none() {
                return Ok(());
            }
            <RunAction as Component>::render::<B>(
                &mut self.run_action_pane,
                frame,
                all[0],
                self.active_area,
            )?;

            //(self.run_action_pane as &mut dyn Component)
            //    .render::<B>(frame, all[0], self.active_area)?;
            return Ok(());
        }

        // render status bar
        frame.render_widget(status_bar(self.active_area), all[1]);

        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Percentage(30), Percentage(70), Min(0)])
            .split(all[0]);

        // rendering projects
        let mut project_list = ProjectList {
            projects: &mut self.projects,
        };
        project_list.render::<B>(frame, main_layout[0], self.active_area)?;

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Percentage(50),
                Percentage(50), // examples
            ])
            .split(main_layout[1]);

        // render action list
        let mut action_list = ActionList {
            actions: &mut self.actions,
        };
        action_list.render::<B>(frame, right_layout[0], self.active_area)?;

        // updating text areas props
        if let Some(action) = &self.current_action {
            if self.action_has_changed {
                self.action_text_areas.action = Some(action.clone());
                self.action_text_areas.clear_text_areas = true;
                self.action_text_areas
                    .render(frame, right_layout[1], self.active_area)?;
                self.action_has_changed = false;

                self.run_action_pane.on_new_action(action.clone());
            } else {
                self.action_text_areas
                    .render(frame, right_layout[1], self.active_area)?;
            }
        }

        Ok(())
    }
}

impl UIRunner for App<'_> {
    fn handle_event(&mut self) -> io::Result<bool> {
        // polling for an event
        if !event::poll(Duration::from_millis(16))? {
            return Ok(false);
        }

        let ev = event::read()?;
        match ev.into() {
            Input { key: Key::Esc, .. } => match self.active_area {
                ActiveArea::BodyExample | ActiveArea::ResponseExample | ActiveArea::RunAction => {
                    self.active_area = ActiveArea::ActionPane;
                }
                // early escape
                _ => return Ok(true),
            },
            input @ Input {
                key: Key::Right,
                ctrl: false,
                alt: false,
                shift: false,
            } => match &mut self.active_area {
                ActiveArea::ProjectPane => {
                    self.active_area = ActiveArea::ActionPane;
                }
                ActiveArea::BodyExample => {
                    let _ = self
                        .action_text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self
                        .action_text_areas
                        .right_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::RunAction => {
                    let active_text_area = match &self.run_action_pane.active_text_area {
                        ActiveTextArea::Edit => &mut self.run_action_pane.edit_text_area,
                        ActiveTextArea::ResponseBody => {
                            &mut self.run_action_pane.response_body_text_area
                        }
                        ActiveTextArea::ResponseHeaders => {
                            &mut self.run_action_pane.response_headers_text_area
                        }
                    };

                    let _ = active_text_area.get_text_area_mut().input(input);
                }
                _ => {}
            },
            input @ Input {
                key: Key::Left,
                ctrl: false,
                alt: false,
                shift: false,
            } => match &mut self.active_area {
                ActiveArea::ActionPane => {
                    self.active_area = ActiveArea::ProjectPane;
                }
                ActiveArea::BodyExample => {
                    let _ = self
                        .action_text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self
                        .action_text_areas
                        .right_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::RunAction => {
                    let active_text_area = match &self.run_action_pane.active_text_area {
                        ActiveTextArea::Edit => &mut self.run_action_pane.edit_text_area,
                        ActiveTextArea::ResponseBody => {
                            &mut self.run_action_pane.response_body_text_area
                        }
                        ActiveTextArea::ResponseHeaders => {
                            &mut self.run_action_pane.response_headers_text_area
                        }
                    };

                    let _ = active_text_area.get_text_area_mut().input(input);
                }
                _ => {}
            },
            input @ Input {
                key: Key::Up,
                ctrl: false,
                alt: false,
                shift: false,
            } => match &mut self.active_area {
                ActiveArea::ProjectPane => {
                    self.projects.previous();
                    self.update_actions();
                }
                ActiveArea::ActionPane => {
                    self.actions.previous();
                    self.set_current_action();
                }
                ActiveArea::BodyExample => {
                    let _ = self
                        .action_text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self
                        .action_text_areas
                        .right_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::RunAction => {
                    let active_text_area = match &self.run_action_pane.active_text_area {
                        ActiveTextArea::Edit => &mut self.run_action_pane.edit_text_area,
                        ActiveTextArea::ResponseBody => {
                            &mut self.run_action_pane.response_body_text_area
                        }
                        ActiveTextArea::ResponseHeaders => {
                            &mut self.run_action_pane.response_headers_text_area
                        }
                    };

                    let _ = active_text_area.get_text_area_mut().input(input);
                }
                _ => {}
            },
            input @ Input {
                key: Key::Down,
                ctrl: false,
                alt: false,
                shift: false,
            } => match self.active_area {
                ActiveArea::ProjectPane => {
                    self.projects.next();
                    self.update_actions();
                }
                ActiveArea::ActionPane => {
                    self.actions.next();
                    self.set_current_action();
                }
                ActiveArea::BodyExample => {
                    let _ = self
                        .action_text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::ResponseExample => {
                    let _ = self
                        .action_text_areas
                        .right_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::RunAction => {
                    let active_text_area = match &self.run_action_pane.active_text_area {
                        ActiveTextArea::Edit => &mut self.run_action_pane.edit_text_area,
                        ActiveTextArea::ResponseBody => {
                            &mut self.run_action_pane.response_body_text_area
                        }
                        ActiveTextArea::ResponseHeaders => {
                            &mut self.run_action_pane.response_headers_text_area
                        }
                    };

                    let _ = active_text_area.get_text_area_mut().input(input);
                }
                _ => {}
            },

            // All others inputs
            input => match self.active_area {
                ActiveArea::ResponseExample => {
                    let _ = self
                        .action_text_areas
                        .right_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                // Text area add char to the text area
                ActiveArea::BodyExample => {
                    let _ = self
                        .action_text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                ActiveArea::RunAction => match input {
                    Input {
                        key: Key::Char('r'),
                        ctrl: true,
                        ..
                    } => {
                        if self.run_action_pane.active_text_area == ActiveTextArea::Edit {
                            self.run_action()
                        }
                    }
                    _ => {
                        let active_text_area = match &self.run_action_pane.active_text_area {
                            ActiveTextArea::Edit => &mut self.run_action_pane.edit_text_area,
                            ActiveTextArea::ResponseBody => {
                                &mut self.run_action_pane.response_body_text_area
                            }
                            ActiveTextArea::ResponseHeaders => {
                                &mut self.run_action_pane.response_headers_text_area
                            }
                        };

                        let _ = active_text_area.get_text_area_mut().input(input);
                    }
                },

                // Action pane
                ActiveArea::ActionPane => match input {
                    // Run action widget
                    Input {
                        key: Key::Enter, ..
                    } => {
                        if self.current_action.is_some() {
                            self.active_area = ActiveArea::RunAction
                        }
                    }

                    // go to response example widget
                    Input {
                        key: Key::Char('r'),
                        ctrl: true,
                        ..
                    } => self.active_area = ActiveArea::ResponseExample,

                    // go to body example widget
                    Input {
                        key: Key::Char('b'),
                        ctrl: true,
                        ..
                    } => self.active_area = ActiveArea::BodyExample,

                    // quit app early
                    Input {
                        key: Key::Char('q'),
                        ctrl: false,
                        alt: false,
                        shift: false,
                    } => return Ok(true),
                    _ => {}
                },

                // otherwise
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

    fn ui<B: Backend>(&mut self, f: &mut Frame) {
        match self.rx.try_recv() {
            Ok(message) => match message {
                Message::RunResult(r) => {
                    // get last domain action results only. Means that  when we have
                    // chained actions we keep the last one only.
                    // Then, we have a vector of R which represents the fetch results
                    // of cartesian product of all parameters (several urls) and an associated
                    // vector of all tests results (several expect tests possible)
                    let (fetch_results, test_result) = r.into_iter().last().unwrap();

                    // assuming that we keep the last fetch result
                    let fetch_result = fetch_results.into_iter().last().unwrap();
                    self.run_action_pane.response_body_text_area.set_text_inner(
                        &fetch_result
                            .result
                            .as_ref()
                            .ok()
                            .and_then(|r| {
                                serde_json::from_str::<Value>(&r.response)
                                    .and_then(|r| serde_json::to_string_pretty(&r))
                                    .ok()
                            })
                            .unwrap_or("".to_string()),
                    );
                    self.run_action_pane
                        .response_headers_text_area
                        .set_text_inner(
                            &fetch_result
                                .result
                                .as_ref()
                                .ok()
                                .map(|r| &r.headers)
                                .and_then(|h| serde_json::to_string_pretty(h).ok())
                                .unwrap_or("".to_string()),
                        );
                    self.run_action_pane.fetch_result = Some(fetch_result);
                    self.run_action_pane.test_results =
                        Some(test_result.into_iter().last().unwrap_or(vec![]));
                }
                // load other actions
                Message::UpdateAction(actions) => {
                    self.actions.items = actions;
                }
            },
            Err(_) => {}
        }
        let r = self.build_ui::<B>(f);
        if let Err(e) = r {
            println!("Error: {}", e);
        }
    }
}
