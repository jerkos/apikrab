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
    text_area, ActionTextAreas, DisplayFromAction, Examples, TextArea,
};
use super::components::project_list::ProjectList;
use super::components::run_action::{RunAction, RunStatus};
use super::event::Event;
use super::helpers::{centered_rect, Component};
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActiveArea {
    #[default]
    ProjectPane,
    ActionPane,
    NewActionPopup,
    BodyExample,
    ResponseExample,
    // entire screen
    RunAction,
}

#[derive(Debug)]
pub enum Move {
    Up,
    Down,
}

#[derive(Debug)]
pub enum Message {
    SwitchArea(ActiveArea),
    RunResult(Vec<(Vec<R>, Vec<Vec<UnaryTestResult>>)>),
    UpdateActions(Vec<Action>),
    CreateAction,
    ActionSaved(Action),
    ListMove(Move),
    DeleteAction,
    ActionDeleted,
    SaveAction,
    RunAction,
    InputTextArea(Input),
    ForwardEvent(Input),
}

fn send_message(tx: mpsc::Sender<Message>, message: Message) {
    tokio::spawn(async move {
        let _ = tx.send(message).await;
    });
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
    action_has_changed: bool,
    new_action_text_area: TextArea<'a>,
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
            run_action_pane: RunAction::new(
                extension,
                text_area(""),
                text_area(""),
                text_area(""),
                text_area(""),
                text_area(""),
                text_area(""),
            ),
            action_has_changed: false,
            new_action_text_area: text_area("New action"),
            tx,
            rx,
        }
    }

    pub fn input_text_area(&mut self, input: Input) {
        match self.active_area {
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
            ActiveArea::NewActionPopup => {
                let _ = self.new_action_text_area.get_text_area_mut().input(input);
            }
            _ => {}
        }
    }

    pub fn list_move(&mut self, m: Move) {
        match m {
            Move::Up => {
                if self.active_area == ActiveArea::ProjectPane {
                    self.projects.previous();
                    self.update_displayed_actions();
                } else if self.active_area == ActiveArea::ActionPane {
                    self.actions.previous();
                    self.action_has_changed = true;
                }
            }
            Move::Down => {
                if self.active_area == ActiveArea::ProjectPane {
                    self.projects.next();
                    self.update_displayed_actions();
                } else if self.active_area == ActiveArea::ActionPane {
                    self.actions.next();
                    self.action_has_changed = true;
                }
            }
        }
    }

    /// Update the actions list
    /// This function will spawn a new tokio task to fetch the actions
    fn update_displayed_actions(&mut self) {
        let db = self.db.clone();
        let tx = self.tx.clone();
        let active_project = &self.projects.items[self.projects.state.selected().unwrap()];
        let project_name = active_project.name.clone();
        tokio::spawn(async move {
            let actions = db.get_actions(Some(&project_name)).await.unwrap();
            tx.send(Message::UpdateActions(actions)).await.unwrap();
        });
    }

    fn forward_event(&mut self, input: Input) {
        if let ActiveArea::RunAction = self.active_area {
            self.run_action_pane.handle_event(input, self.tx.clone());
        }
    }

    fn switch_area(&mut self, area: ActiveArea) {
        self.active_area = area;
    }

    /// create new action
    fn create_action(&mut self) {
        let action = Action {
            name: Some(self.new_action_text_area.get_text_content().to_string()),
            project_name: Some(
                self.projects.items[self.projects.state.selected().unwrap()]
                    .name
                    .clone(),
            ),
            actions: vec![],
            created_at: Some(chrono::Local::now().naive_local()),
            ..Default::default()
        };
        let action_cloned = action.clone();
        self.actions.items.push(action);
        let actions_len = self.actions.items.len() - 1;
        self.run_action_pane
            .on_new_action(&action_cloned, self.db.get_serializer());

        self.active_area = ActiveArea::RunAction;
        self.actions.state.select(Some(actions_len));
    }

    fn delete_action(&mut self) {
        let action = self.get_current_action();
        if let Some(action) = action {
            let db = self.db.clone();
            let tx = self.tx.clone();
            let action_name_cloned = action.name.clone();
            let project_name_cloned = action.project_name.clone();
            tokio::spawn(async move {
                let _ = db
                    .rm_action(
                        action_name_cloned.as_deref().unwrap(),
                        project_name_cloned.as_deref(),
                    )
                    .await;
                let _ = tx.send(Message::ActionDeleted).await;
            });
        }
    }

    fn action_deleted(&mut self) {
        let idx = self.actions.state.selected();
        if let Some(idx) = idx {
            self.actions.items.remove(idx);
        }
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

        let mut action = self.get_current_action().unwrap().clone();
        action.actions = domain_actions.unwrap();

        let db = self.db.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            db.upsert_action(&action).await.unwrap();
            tx.send(Message::ActionSaved(action.clone())).await.unwrap();
        });
    }

    fn action_saved(&mut self, action: Action) {
        let selected_index = self.actions.state.selected();
        if let Some(idx) = selected_index {
            let items = &mut self.actions.items;
            let _ = std::mem::replace(&mut items[idx], action);
            self.run_action_pane.changed_content_not_saved = false;
        }
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
                let api = Api::new(Some(action.timeout), action.insecure);
                let r = action
                    .run_with_tests(None, &mut ctx, &*db, &api, &mut printer, &multi, &pb)
                    .await;
                results.push(r);
            }
            let _ = tx.send(Message::RunResult(results)).await;
        });
    }

    fn get_current_action(&self) -> Option<&Action> {
        self.actions
            .state
            .selected()
            .and_then(|idx| self.actions.items.get(idx))
    }

    /// build the UI
    pub fn build_ui(&mut self, frame: &mut Frame) -> io::Result<()> {
        // run action frame
        if self.active_area == ActiveArea::RunAction {
            if self.get_current_action().is_none() {
                return Ok(());
            }
            <RunAction as Component>::render(
                &mut self.run_action_pane,
                frame,
                frame.size(),
                self.active_area,
            )?;
            return Ok(());
        }

        // other frames
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Percentage(30), Percentage(70), Min(0)])
            .split(frame.size());

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

        let action = self.get_current_action();
        let action_clone = action.cloned();
        // updating text areas props
        if let Some(action) = action_clone {
            if self.action_has_changed {
                self.action_text_areas.left_text = action.body_example.clone();
                self.action_text_areas.right_text = action.response_example.clone();
                self.action_text_areas.clear_text_areas = true;

                self.action_text_areas
                    .render(frame, right_layout[1], self.active_area)?;
                self.action_has_changed = false;

                self.run_action_pane
                    .on_new_action(&action, self.db.get_serializer());
            } else {
                self.action_text_areas
                    .render(frame, right_layout[1], self.active_area)?;
            }
        }

        if self.active_area == ActiveArea::NewActionPopup {
            let area = centered_rect(60, 20, frame.size());
            frame.render_widget(Clear, area);
            frame.render_widget(self.new_action_text_area.get_text_area().widget(), area);
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
                ActiveArea::NewActionPopup => match ev {
                    Input { key: Key::Esc, .. } => {
                        send_message(self.tx.clone(), Message::SwitchArea(ActiveArea::ActionPane));
                    }
                    Input {
                        key: Key::Enter, ..
                    } => {
                        send_message(self.tx.clone(), Message::CreateAction);
                    }
                    input => {
                        send_message(self.tx.clone(), Message::InputTextArea(input));
                    }
                },
                ActiveArea::RunAction => send_message(self.tx.clone(), Message::ForwardEvent(ev)),
                ActiveArea::ProjectPane => match ev {
                    Input { key: Key::Esc, .. } => return Ok(true),
                    Input {
                        key: Key::Right, ..
                    } => send_message(self.tx.clone(), Message::SwitchArea(ActiveArea::ActionPane)),
                    Input { key: Key::Up, .. } => {
                        send_message(self.tx.clone(), Message::ListMove(Move::Up));
                    }
                    Input { key: Key::Down, .. } => {
                        send_message(self.tx.clone(), Message::ListMove(Move::Down));
                    }
                    _ => {}
                },
                ActiveArea::ActionPane => {
                    match ev {
                        Input { key: Key::Esc, .. } => return Ok(true),
                        Input { key: Key::Left, .. } => send_message(
                            self.tx.clone(),
                            Message::SwitchArea(ActiveArea::ProjectPane),
                        ),
                        Input { key: Key::Up, .. } => {
                            send_message(self.tx.clone(), Message::ListMove(Move::Up));
                        }
                        Input { key: Key::Down, .. } => {
                            send_message(self.tx.clone(), Message::ListMove(Move::Down));
                        }
                        Input {
                            key: Key::Enter, ..
                        } => {
                            if self.get_current_action().is_some() {
                                send_message(
                                    self.tx.clone(),
                                    Message::SwitchArea(ActiveArea::RunAction),
                                );
                            }
                        }
                        Input {
                            key: Key::Char('r'),
                            ctrl: true,
                            ..
                        } => send_message(
                            self.tx.clone(),
                            Message::SwitchArea(ActiveArea::ResponseExample),
                        ),

                        // go to body example widget
                        Input {
                            key: Key::Char('b'),
                            ctrl: true,
                            ..
                        } => send_message(
                            self.tx.clone(),
                            Message::SwitchArea(ActiveArea::BodyExample),
                        ),
                        Input {
                            key: Key::Char('n'),
                            ctrl: true,
                            ..
                        } => send_message(
                            self.tx.clone(),
                            Message::SwitchArea(ActiveArea::NewActionPopup),
                        ),
                        Input {
                            key: Key::Char('d'),
                            ctrl: true,
                            ..
                        } => {
                            if self.active_area != ActiveArea::NewActionPopup {
                                send_message(self.tx.clone(), Message::DeleteAction)
                            }
                        }
                        _ => {}
                    }
                }
                ActiveArea::BodyExample | ActiveArea::ResponseExample => match ev {
                    Input { key: Key::Esc, .. } => {
                        send_message(self.tx.clone(), Message::SwitchArea(ActiveArea::ActionPane))
                    }
                    input => send_message(self.tx.clone(), Message::InputTextArea(input)),
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
                Message::UpdateActions(actions) => self.actions.items = actions,
                Message::SaveAction => self.save_action(),
                Message::RunAction => self.run_action(),
                Message::DeleteAction => self.delete_action(),
                Message::ActionDeleted => self.action_deleted(),
                Message::SwitchArea(area) => self.switch_area(area),
                Message::ListMove(m) => self.list_move(m),
                Message::InputTextArea(input) => self.input_text_area(input),
                Message::CreateAction => self.create_action(),
                Message::ForwardEvent(input) => self.forward_event(input),
            }
        }
        let r = self.build_ui(f);
        if let Err(e) = r {
            println!("Error: {}", e);
        }
    }
}
