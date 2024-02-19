use crate::commands::run::_printer::Printer;
use crate::commands::run::_progress_bar::init_progress_bars;
use crate::commands::run::_test_checker::UnaryTestResult;
use crate::commands::run::action::R;
use crate::db::db_trait::Db;
use crate::db::dto::{Action, Project};
use crate::http::Api;
use crate::ui::helpers::{Stateful, StatefulList};
use apikrab::serializer::SerDe;
use indicatif::ProgressDrawTarget;
use ratatui::Frame;
use ratatui::{layout::Constraint::*, prelude::*, widgets::*};
use std::collections::HashMap;
use std::{io, vec};
use tokio::sync::mpsc;
use tui_textarea::{Input, Key};

use super::components::action_list::ActionList;
use super::components::action_text_areas::{
    text_area, ActionTextAreas, DisplayFromAction, Examples,
};
use super::components::project_list::ProjectList;
use super::components::run_action::{RunAction, RunStatus, TestStatus};
use super::components::status_bar::status_bar;
use super::custom_renderer;
use super::event::Event;
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
    ActionSaved(Action),
    SaveAction,
    RunAction,
    LeaveRunAction,
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
    pub fn new(projects: Vec<Project>, db: Box<dyn Db>) -> Self {
        let (tx, rx) = mpsc::channel(200);
        // get extension of the current serializer
        let extension = db
            .get_serializer()
            .map(|s| s.ending())
            .unwrap_or("toml".to_string());
        Self {
            db,
            active_area: ActiveArea::ProjectPane,
            projects: StatefulList::with_items(projects),
            actions: StatefulList {
                state: ListState::default(),
                items: Vec::new(),
            },
            action_text_areas: ActionTextAreas::new("Body", "Response", &EXAMPLE),
            // todo create a new function to init run action pane
            // with sane default
            run_action_pane: RunAction {
                active_text_area: Default::default(),
                edit_extension: extension,
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
                changed_content_not_saved: false,
            },
            current_action: None,
            action_has_changed: false,
            tx,
            rx,
        }
    }

    /// Update the actions list
    /// This function will spawn a new tokio task to fetch the actions
    fn update_displayed_actions(&mut self) {
        let projects = self.projects.clone();
        let db = self.db.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let selected_item = projects.items[projects.state.selected().unwrap()].clone();
            let actions = db.get_actions(Some(&selected_item.name)).await.unwrap();
            tx.send(Message::UpdateAction(actions)).await.unwrap();
        });
    }

    /// Save the current action
    fn save_action(&mut self) {
        let domain_actions = self
            .db
            .get_serializer()
            .and_then(|ser| self.run_action_pane.get_action_from_text_content(ser));

        if domain_actions.is_none() {
            return;
        }

        let mut action = self.current_action.as_ref().unwrap().clone();
        action.actions = domain_actions.unwrap();

        let db = self.db.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            db.upsert_action(&action).await.unwrap();
            tx.send(Message::ActionSaved(action)).await.unwrap();
        });
    }

    fn action_saved(&mut self, action: Action) {
        let items = &mut self.actions.items;
        let selected_index = self.actions.state.selected().unwrap();
        let _ = std::mem::replace(&mut items[selected_index], action);
        self.run_action_pane.changed_content_not_saved = false;
    }

    /// Run the current action
    /// This function will spawn a new tokio task to run the action
    fn run_action(&mut self) {
        // updating run action pane
        self.run_action_pane
            .reset_response_with_status(RunStatus::Running);

        // get domain actions from the text area
        let domain_actions = self
            .db
            .get_serializer()
            .and_then(|ser| self.run_action_pane.get_action_from_text_content(ser));

        if domain_actions.is_none() {
            return;
        }

        let actions = domain_actions.unwrap();

        // build all necessary stuff to run the action
        let api = Api::new(Some(5), false);
        let mut ctx = HashMap::new();
        let (multi, pb) = init_progress_bars(actions.len() as u64);
        multi.set_draw_target(ProgressDrawTarget::hidden());
        pb.set_draw_target(ProgressDrawTarget::hidden());
        let mut printer = Printer::new(true, false, false);

        // cloning all necessary stuff to run the action
        let db = self.db.clone();
        let tx = self.tx.clone();

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

    /// Set the current action
    fn set_current_action(&mut self) {
        self.current_action =
            Some(self.actions.items[self.actions.state.selected().unwrap()].clone());
        self.action_has_changed = true;
    }

    /// build the UI
    pub fn build_ui(&mut self, frame: &mut Frame) -> io::Result<()> {
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
            <RunAction as Component>::render(
                &mut self.run_action_pane,
                frame,
                all[0],
                self.active_area,
            )?;

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
        project_list.render(frame, main_layout[0], self.active_area)?;

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
        action_list.render(frame, right_layout[0], self.active_area)?;

        // updating text areas props

        if let Some(action) = &self.current_action {
            if self.action_has_changed {
                //self.action_text_areas.action = Some(action.clone());
                //self.action_text_areas.clear_text_areas = true;
                //self.action_text_areas
                //    .render(frame, right_layout[1], self.active_area)?;
                self.action_has_changed = false;

                self.run_action_pane
                    .on_new_action(action.clone(), self.db.get_serializer());
            } else {
                //self.action_text_areas
                //    .render(frame, right_layout[1], self.active_area)?;
            }
        }

        Ok(())
    }

    pub fn handle_event(&mut self, event: &Event) -> io::Result<bool> {
        // polling for an event
        if let Event::Error = event {
            return Ok(false);
        }
        if let Event::Key(key_event) = event {
            let ev: Input = Input::from(*key_event);
            match self.active_area {
                ActiveArea::RunAction => self.run_action_pane.handle_event(ev, self.tx.clone()),
                ActiveArea::ProjectPane => match ev {
                    Input { key: Key::Esc, .. } => return Ok(true),
                    Input {
                        key: Key::Right, ..
                    } => self.active_area = ActiveArea::ActionPane,
                    Input { key: Key::Up, .. } => {
                        self.projects.previous();
                        self.update_displayed_actions();
                    }
                    Input { key: Key::Down, .. } => {
                        self.projects.next();
                        self.update_displayed_actions();
                    }
                    _ => {}
                },
                ActiveArea::ActionPane => match ev {
                    Input { key: Key::Esc, .. } => return Ok(true),
                    Input { key: Key::Left, .. } => self.active_area = ActiveArea::ProjectPane,
                    Input { key: Key::Up, .. } => {
                        self.actions.previous();
                        self.set_current_action();
                    }
                    Input { key: Key::Down, .. } => {
                        self.actions.next();
                        self.set_current_action();
                    }
                    Input {
                        key: Key::Enter, ..
                    } => {
                        if self.current_action.is_some() {
                            self.active_area = ActiveArea::RunAction
                        }
                    }
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
                    _ => {}
                },
                ActiveArea::BodyExample | ActiveArea::ResponseExample => match ev {
                    Input { key: Key::Esc, .. } => self.active_area = ActiveArea::ActionPane,
                    input => match self.active_area {
                        ActiveArea::ResponseExample => {
                            let _ = self
                                .action_text_areas
                                .right_text_area
                                .get_text_area_mut()
                                .input(input);
                        }
                        ActiveArea::BodyExample => {
                            let _ = self
                                .action_text_areas
                                .left_text_area
                                .get_text_area_mut()
                                .input(input);
                        }
                        _ => {}
                    },
                },
            }
        }

        Ok(false)
    }

    pub fn ui(&mut self, f: &mut Frame) {
        if let Ok(message) = self.rx.try_recv() {
            match message {
                Message::ActionSaved(action) => self.action_saved(action),
                Message::RunResult(r) => self.run_action_pane.on_run_action_result(r),
                // load other actions
                Message::UpdateAction(actions) => self.actions.items = actions,
                Message::SaveAction => self.save_action(),
                Message::RunAction => self.run_action(),
                Message::LeaveRunAction => self.active_area = ActiveArea::ActionPane,
            }
        }
        let r = self.build_ui(f);
        if let Err(e) = r {
            println!("Error: {}", e);
        }
    }
}
