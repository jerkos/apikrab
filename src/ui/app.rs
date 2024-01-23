use crate::commands::run::_run_helper::ANONYMOUS_ACTION;
use crate::db::db_trait::Db;
use crate::db::dto::{Action, Project};
use crate::ui::components::action_text_areas::DomainActions;
use crate::ui::helpers::{Stateful, StatefulList};
use crate::ui::run_ui::UIRunner;
use crossterm::event::{self};
use ratatui::backend::Backend;
use ratatui::Frame;
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};
use std::{io, thread};
use tokio::runtime::Handle;
use tui_textarea::{Input, Key};

use super::components::action_list::ActionList;
use super::components::action_text_areas::{ActionTextAreas, DisplayFromAction, Examples};
use super::components::project_list::ProjectList;
use super::components::run_action::RunAction;
use super::components::status_bar::status_bar;
use super::helpers::Component;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActiveArea {
    #[default]
    ProjectPane,
    ActionPane,
    BodyExample,
    ResponseExample,
    RunAction,
    DomainAction,
    Result,
}

#[derive(Clone)]
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
}

lazy_static! {
    pub static ref EXAMPLE: Box<dyn DisplayFromAction> = Box::new(Examples {});
    pub static ref DOMAIN_ACTION: Box<dyn DisplayFromAction> = Box::new(DomainActions {});
}

impl<'a> App<'a> {
    pub fn new(projects: Vec<Project>, db_handler: Box<dyn Db>) -> Self {
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
                text_areas: ActionTextAreas::new("Action", "Response", &DOMAIN_ACTION),
            },
            current_action: None,
            action_has_changed: false,
        }
    }

    fn update_actions(&mut self) {
        let handle = Handle::current();
        let self_cloned = self.clone();

        let actions = thread::spawn(move || {
            handle.block_on(async move {
                let projects = self_cloned.projects;
                let selected_item = projects.items[projects.state.selected().unwrap()].clone();
                self_cloned
                    .db
                    .get_actions(Some(&selected_item.name))
                    .await
                    .unwrap()
            })
        })
        .join()
        .unwrap();
        self.actions.items = actions;
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

        if self.active_area == ActiveArea::RunAction
            || self.active_area == ActiveArea::Result
            || self.active_area == ActiveArea::DomainAction
        {
            if self.current_action.is_none() {
                return Ok(());
            }
            //run_action.text_areas.clear_text_areas = true;
            //run_action.text_areas.action = self.current_action.clone();
            self.run_action_pane
                .render::<B>(frame, all[0], self.active_area)?;
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
                    .render::<B>(frame, right_layout[1], self.active_area)?;
                self.action_has_changed = false;

                self.run_action_pane.text_areas.clear_text_areas = true;
                self.run_action_pane.text_areas.action = self.current_action.clone();
            } else {
                self.action_text_areas
                    .render::<B>(frame, right_layout[1], self.active_area)?;
            }
        }

        Ok(())
    }
}

impl UIRunner for App<'_> {
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
                ActiveArea::DomainAction => {
                    let _ = self
                        .run_action_pane
                        .text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
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
                ActiveArea::DomainAction => {
                    let _ = self
                        .run_action_pane
                        .text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
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
                ActiveArea::DomainAction => {
                    let _ = self
                        .run_action_pane
                        .text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
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
                ActiveArea::DomainAction => {
                    let _ = self
                        .run_action_pane
                        .text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
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
                ActiveArea::DomainAction => {
                    let _ = self
                        .run_action_pane
                        .text_areas
                        .left_text_area
                        .get_text_area_mut()
                        .input(input);
                }
                // Action pane
                ActiveArea::ActionPane => match input {
                    // Run action widget
                    Input {
                        key: Key::Char('g'),
                        ctrl: true,
                        ..
                    } => self.active_area = ActiveArea::DomainAction,

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
        let r = self.build_ui::<B>(f);
        if let Err(e) = r {
            println!("Error: {}", e);
        }
    }
}
