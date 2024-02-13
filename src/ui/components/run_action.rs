use std::io;

use ratatui::{layout::*, prelude::*, widgets::*};

use crate::{
    commands::run::{_run_helper::ANONYMOUS_ACTION, _test_checker::UnaryTestResult, action::R},
    db::dto::Action,
    ui::{
        app::ActiveArea,
        custom_renderer::{self, Renderer},
        helpers::Component,
    },
};

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

//#[derive(Clone)]
pub struct RunAction<'a> {
    pub action_name: Option<String>,
    pub project_name: Option<String>,

    pub active_text_area: ActiveTextArea,

    pub edit_text_area: TextArea<'a>,
    pub edit_text_area_viewport: custom_renderer::Viewport,

    pub response_body_text_area: TextArea<'a>,
    pub response_body_text_area_viewport: custom_renderer::Viewport,

    pub response_headers_text_area: TextArea<'a>,
    pub response_headers_text_area_viewport: custom_renderer::Viewport,

    pub status: RunStatus,
    pub test_status: TestStatus,
    pub test_results: Option<Vec<UnaryTestResult>>,
    pub fetch_result: Option<R>,
}

impl RunAction<'_> {
    pub fn on_new_action(&mut self, action: Action) {
        self.action_name = action
            .name
            .clone()
            .or_else(|| ANONYMOUS_ACTION.to_owned().into());

        self.project_name = action
            .project_name
            .clone()
            .or_else(|| ANONYMOUS_ACTION.to_owned().into());

        let value = serde_json::to_string_pretty(&action.actions).unwrap_or("".to_string());
        self.edit_text_area.clear_text_area();
        self.edit_text_area.set_text_inner(&value);
        self.response_body_text_area.clear_text_area();
        self.response_headers_text_area.clear_text_area();
        self.status = RunStatus::Idle;
        self.fetch_result = None;
        self.test_status = TestStatus::NotRun;
        self.test_results = None;
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
                        format!("{} ", self.action_name.clone().unwrap_or_default()),
                        Style::default().yellow().bold(),
                    ),
                ]),
                Line::from(vec![Span::raw("\nProject: ")]),
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
}

impl Component for RunAction<'_> {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _: ActiveArea,
    ) -> io::Result<()> {
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

        self.edit_text_area
            .get_text_area_mut()
            .set_block(Block::new().title("Edit").borders(Borders::ALL).green());

        let edit_text_area_renderer = Renderer {
            text_area: &self.edit_text_area,
            viewport: &self.edit_text_area_viewport,
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
        };

        frame.render_widget(response_body_text_area_renderer, result_text_area[0]);

        let response_headers_text_area_renderer = Renderer {
            text_area: &self.response_headers_text_area,
            viewport: &self.response_headers_text_area_viewport,
        };

        frame.render_widget(response_headers_text_area_renderer, result_text_area[1]);

        frame.render_widget(self.test_results(), result_area[1]);

        Ok(())
    }
}
