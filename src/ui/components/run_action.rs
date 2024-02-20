use std::io;

use apikrab::serializer::SerDe;
use ratatui::{layout::*, prelude::*, widgets::*};
use serde_json::Value;
use tokio::sync::mpsc::Sender;

use crate::{
    commands::run::{_run_helper::ANONYMOUS_ACTION, _test_checker::UnaryTestResult, action::R},
    db::{db_trait::FileTypeSerializer, dto::Action},
    domain::{DomainAction, DomainActions},
    ui::{
        app::{ActiveArea, Message},
        custom_renderer::{self, Renderer},
        helpers::Component,
    },
};

use tui_textarea::{Input, Key};

use super::action_text_areas::TextArea;

#[derive(Clone, Default)]
pub enum RunStatus {
    #[default]
    Idle,
    Running,
    Failed,
    Success,
}

impl ToString for RunStatus {
    fn to_string(&self) -> String {
        match self {
            RunStatus::Idle => "Idle".to_owned(),
            RunStatus::Running => "Running".to_owned(),
            RunStatus::Failed => "Failed".to_owned(),
            RunStatus::Success => "Success".to_owned(),
        }
    }
}

#[derive(Clone, Default)]
pub enum TestStatus {
    #[default]
    NotRun,
    Failed,
    Success,
}

#[derive(Default, PartialEq)]
pub enum ActiveTextArea {
    #[default]
    Edit,
    ResponseBody,
    ResponseHeaders,
}

pub struct RunAction<'a> {
    /// The action to run
    pub action_name: Option<String>,
    /// The project name of the action
    pub project_name: Option<String>,

    /// The active text area
    pub active_text_area: ActiveTextArea,

    /// the extension of the file being edited for syntax highlighting
    pub edit_extension: String,

    /// The text area for editing the action
    pub edit_text_area: TextArea<'a>,
    pub edit_text_area_viewport: custom_renderer::Viewport,

    /// The text area for the response body
    pub response_body_text_area: TextArea<'a>,
    pub response_body_text_area_viewport: custom_renderer::Viewport,

    /// The text area for the response headers
    pub response_headers_text_area: TextArea<'a>,
    pub response_headers_text_area_viewport: custom_renderer::Viewport,

    /// The status of the run
    pub status: RunStatus,
    /// The status of the tests
    pub test_status: TestStatus,

    /// The results of the tests
    pub test_results: Option<Vec<UnaryTestResult>>,

    /// The result of the fetch
    pub fetch_result: Option<R>,

    /// Whether the content has been changed and not saved
    pub changed_content_not_saved: bool,
}

impl<'a> RunAction<'a> {
    pub fn new(
        edit_extension: String,
        edit_text_area: TextArea<'a>,
        response_body_text_area: TextArea<'a>,
        response_headers_text_area: TextArea<'a>,
    ) -> RunAction<'a> {
        Self {
            edit_text_area,
            response_body_text_area,
            response_headers_text_area,
            action_name: None,
            project_name: None,
            active_text_area: ActiveTextArea::default(),
            edit_extension,
            edit_text_area_viewport: custom_renderer::Viewport::default(),
            response_body_text_area_viewport: custom_renderer::Viewport::default(),
            response_headers_text_area_viewport: custom_renderer::Viewport::default(),
            status: RunStatus::default(),
            test_status: TestStatus::default(),
            test_results: None,
            fetch_result: None,
            changed_content_not_saved: false,
        }
    }

    pub fn reset_response_with_status(&mut self, status: RunStatus) {
        self.response_body_text_area.clear_text_area();
        self.response_headers_text_area.clear_text_area();
        self.status = status;
        self.fetch_result = None;
        self.test_status = TestStatus::NotRun;
        self.test_results = None;
    }

    fn set_active_border_color(&mut self) {
        vec![
            &mut self.edit_text_area,
            &mut self.response_body_text_area,
            &mut self.response_headers_text_area,
        ]
        .into_iter()
        .for_each(|t| {
            let block = t
                .get_text_area_mut()
                .block()
                .unwrap()
                .clone()
                .border_style(Style::default());
            t.get_text_area_mut().set_block(block);
        });

        let active_text_area = match self.active_text_area {
            ActiveTextArea::Edit => &mut self.edit_text_area,
            ActiveTextArea::ResponseBody => &mut self.response_body_text_area,
            ActiveTextArea::ResponseHeaders => &mut self.response_headers_text_area,
        };
        let block = active_text_area
            .get_text_area_mut()
            .block()
            .unwrap()
            .clone()
            .border_style(Style::default().fg(Color::Green));
        active_text_area.get_text_area_mut().set_block(block);
    }

    pub fn on_new_action(&mut self, action: Action, serializer: Option<&FileTypeSerializer>) {
        self.action_name = action
            .name
            .clone()
            .or_else(|| ANONYMOUS_ACTION.to_owned().into());

        self.project_name = action
            .project_name
            .clone()
            .or_else(|| ANONYMOUS_ACTION.to_owned().into());

        let domain_actions: DomainActions = action.actions.into();
        let actions = match serializer {
            Some(s) => s.to_string(&domain_actions).unwrap_or("".to_string()),
            None => "".to_string(),
        };
        self.edit_text_area.clear_text_area();
        self.edit_text_area.set_text_content(&actions);

        self.reset_response_with_status(RunStatus::Idle);
    }

    pub fn on_run_action_result(&mut self, results: Vec<(Vec<R>, Vec<Vec<UnaryTestResult>>)>) {
        // get last domain action results only. Means that  when we have
        // chained actions we keep the last one only.
        // Then, we have a vector of R which represents the fetch results
        // of cartesian product of all parameters (several urls) and an associated
        // vector of all tests results (several expect tests possible)
        let (fetch_results, test_result) = results.into_iter().last().unwrap();

        // assuming that we keep the last fetch result
        let fetch_result = fetch_results.into_iter().last().unwrap();
        self.response_body_text_area.set_text_content(
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
        self.response_headers_text_area.set_text_content(
            &fetch_result
                .result
                .as_ref()
                .ok()
                .map(|r| &r.headers)
                .and_then(|h| serde_json::to_string_pretty(h).ok())
                .unwrap_or("".to_string()),
        );
        self.fetch_result = Some(fetch_result);
        self.test_results = Some(test_result.into_iter().last().unwrap_or(vec![]));
    }

    pub fn get_action_from_text_content(
        &self,
        file_type_serializer: &FileTypeSerializer,
    ) -> Option<Vec<DomainAction>> {
        let actions = self.edit_text_area.get_text_content();
        let wrapper = file_type_serializer
            .from_str::<DomainActions>(&actions)
            .ok();
        wrapper.map(|w| w.actions)
    }

    fn get_fetch_result_status_code(&self) -> u16 {
        self.fetch_result
            .as_ref()
            .and_then(|r| r.result.as_ref().ok())
            .map(|r| r.status)
            .unwrap_or(0)
    }

    pub fn compute_states(&mut self) {
        if let Some(test_results) = &self.test_results {
            if test_results.is_empty() {
                self.test_status = TestStatus::NotRun;
            } else {
                self.test_status = if test_results.iter().all(|r| r.is_success) {
                    TestStatus::Success
                } else {
                    TestStatus::Failed
                };
            }
        }

        if let Some(fetch_result) = &self.fetch_result {
            self.status = match self.test_status {
                TestStatus::Success => RunStatus::Success,
                TestStatus::Failed => RunStatus::Failed,
                TestStatus::NotRun => match &fetch_result.result {
                    Ok(r) => {
                        if r.is_success() {
                            RunStatus::Success
                        } else {
                            RunStatus::Failed
                        }
                    }
                    Err(_) => RunStatus::Failed,
                },
            };
        }
    }

    fn test_results(&self) -> Paragraph<'_> {
        let test_status = match self.test_status {
            TestStatus::NotRun => "Not Run".to_owned(),
            TestStatus::Failed => "Failed".to_owned(),
            TestStatus::Success => "Success".to_owned(),
        };

        let mut test_lines = self
            .test_results
            .as_ref()
            .map(|results| {
                results
                    .iter()
                    .map(|r| {
                        let status = match r.is_success {
                            true => "✅ ",
                            false => "❌ ",
                        };
                        Line::from(vec![
                            Span::raw(status),
                            Span::styled(
                                r.message.clone(),
                                match r.is_success {
                                    true => Style::default().fg(Color::Green),
                                    false => Style::default().fg(Color::Red),
                                },
                            )
                            .bold(),
                            match r.is_success {
                                true => Span::raw(""),
                                false => Span::styled(
                                    format!(
                                        "    Expected {:?} got {:?}",
                                        r.expected.as_deref().unwrap_or("<empty>"),
                                        r.got.as_deref().unwrap_or("<empty>")
                                    ),
                                    match r.is_success {
                                        true => Style::default().fg(Color::Green),
                                        false => Style::default().fg(Color::Red),
                                    },
                                )
                                .italic(),
                            },
                        ])
                    })
                    .collect::<Vec<Line>>()
            })
            .unwrap_or_default();

        test_lines.insert(
            0,
            Line::from(vec![Span::styled(
                test_status.to_uppercase(),
                match self.test_status {
                    TestStatus::NotRun => Style::default().fg(Color::DarkGray),
                    TestStatus::Failed => Style::default().fg(Color::Red),
                    TestStatus::Success => Style::default().fg(Color::Green),
                },
            )]),
        );
        Paragraph::new(test_lines)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false })
            .block(
                Block::new()
                    .title("Test results")
                    .borders(Borders::ALL)
                    .padding(Padding::new(2, 0, 0, 0)),
            )
            .on_dark_gray()
    }

    fn status_bar(&self) -> (Paragraph<'_>, Paragraph<'_>) {
        (
            Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("Action: "),
                    Span::styled(
                        self.action_name.clone().unwrap_or_default().to_string(),
                        Style::default().yellow().bold(),
                    ),
                    Span::styled(
                        format!(
                            " {}",
                            if self.changed_content_not_saved {
                                "(*)"
                            } else {
                                ""
                            }
                        ),
                        Style::default().fg(Color::Red).bold(),
                    ),
                ]),
                Line::from(vec![Span::raw("Project: ")]),
            ])
            .alignment(Alignment::Left)
            .block(Block::new().padding(Padding::new(2, 0, 1, 1)))
            .on_dark_gray(),
            Paragraph::new(Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    match self.status {
                        RunStatus::Idle => "🚦 IDLE".to_owned(),
                        RunStatus::Running => "🏃 RUNNING...".to_owned(),
                        RunStatus::Failed | RunStatus::Success => {
                            format!(
                                "  {}: {}  ",
                                self.status.to_string().to_uppercase(),
                                self.get_fetch_result_status_code()
                            )
                        }
                    },
                    match self.status {
                        RunStatus::Failed => Style::default().white().bold().on_red(),
                        RunStatus::Success => Style::default().white().bold().on_green(),
                        _ => Style::default(),
                    },
                ),
            ]))
            .alignment(Alignment::Right)
            .block(Block::new().padding(Padding::new(0, 2, 1, 1)))
            .on_dark_gray(),
        )
    }

    pub fn handle_event(&mut self, input: Input, tx: Sender<Message>) {
        match input {
            Input { key: Key::Esc, .. } => {
                tokio::spawn(async move {
                    tx.send(Message::LeaveRunAction).await.unwrap();
                });
            }
            Input {
                key: Key::Char('r'),
                ctrl: true,
                ..
            } => {
                if self.active_text_area == ActiveTextArea::Edit {
                    tokio::spawn(async move {
                        tx.send(Message::RunAction).await.unwrap();
                    });
                }
            }
            Input {
                key: Key::Char('s'),
                ctrl: true,
                ..
            } => {
                if self.active_text_area == ActiveTextArea::Edit {
                    tokio::spawn(async move {
                        tx.send(Message::SaveAction).await.unwrap();
                    });
                }
            }
            Input {
                key: Key::Char('a'),
                ctrl: true,
                ..
            } => self.active_text_area = ActiveTextArea::Edit,
            Input {
                key: Key::Char('b'),
                ctrl: true,
                ..
            } => self.active_text_area = ActiveTextArea::ResponseBody,
            Input {
                key: Key::Char('h'),
                ctrl: true,
                ..
            } => self.active_text_area = ActiveTextArea::ResponseHeaders,
            input @ Input { key, .. } => {
                let active_text_area = match &self.active_text_area {
                    ActiveTextArea::Edit => &mut self.edit_text_area,
                    ActiveTextArea::ResponseBody => &mut self.response_body_text_area,
                    ActiveTextArea::ResponseHeaders => &mut self.response_headers_text_area,
                };

                let _ = active_text_area.get_text_area_mut().input(input);
                if self.active_text_area == ActiveTextArea::Edit
                    && key != Key::Left
                    && key != Key::Right
                    && key != Key::Up
                    && key != Key::Down
                {
                    self.changed_content_not_saved = true;
                }
            }
        }
    }
}

/// The component for running an action
impl Component for RunAction<'_> {
    fn render(&mut self, frame: &mut Frame, area: Rect, _: ActiveArea) -> io::Result<()> {
        self.compute_states();

        let layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(vec![
                layout::Constraint::Ratio(1, 10),
                layout::Constraint::Ratio(9, 10),
            ])
            .split(area);

        let header = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(vec![
                layout::Constraint::Percentage(50),
                layout::Constraint::Percentage(50),
            ])
            .split(layout[0]);

        let (left_header, right_header) = self.status_bar();

        frame.render_widget(left_header, header[0]);
        frame.render_widget(right_header, header[1]);

        let editor_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(vec![
                layout::Constraint::Percentage(33),
                layout::Constraint::Percentage(66),
            ])
            .split(layout[1]);

        self.set_active_border_color();

        let edit_text_area_renderer = Renderer {
            text_area: &self.edit_text_area,
            viewport: &self.edit_text_area_viewport,
            extension: "toml",
        };

        frame.render_widget(edit_text_area_renderer, editor_area[0]);

        let result_area = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(vec![
                layout::Constraint::Percentage(50),
                layout::Constraint::Percentage(50),
            ])
            .split(editor_area[1]);

        let result_text_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(vec![
                layout::Constraint::Percentage(50),
                layout::Constraint::Percentage(50),
            ])
            .split(result_area[0]);

        let response_body_text_area_renderer = Renderer {
            text_area: &self.response_body_text_area,
            viewport: &self.response_body_text_area_viewport,
            extension: "json",
        };

        frame.render_widget(response_body_text_area_renderer, result_text_area[0]);

        let response_headers_text_area_renderer = Renderer {
            text_area: &self.response_headers_text_area,
            viewport: &self.response_headers_text_area_viewport,
            extension: "json",
        };

        frame.render_widget(response_headers_text_area_renderer, result_text_area[1]);

        frame.render_widget(self.test_results(), result_area[1]);

        Ok(())
    }
}
