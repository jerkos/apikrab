use std::io;

use ratatui::{layout::*, prelude::*, widgets::*};

use crate::{
    commands::run::_run_helper::ANONYMOUS_ACTION,
    db::dto::Action,
    ui::{app::ActiveArea, helpers::Component},
};

use super::action_text_areas::ActionTextAreas;

#[derive(Clone)]
pub struct RunAction<'a> {
    pub text_areas: ActionTextAreas<'a>,
    pub action_name: Option<String>,
    pub is_running: bool,
    pub status: Option<String>,
}

impl RunAction<'_> {
    pub fn on_new_action(&mut self, action: Action) {
        self.text_areas.clear_text_areas = true;
        self.action_name = action
            .name
            .clone()
            .or_else(|| ANONYMOUS_ACTION.to_owned().into());
        self.text_areas.action = Some(action);
    }

    fn status_bar(&self) -> Paragraph<'_> {
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} ", self.action_name.clone().unwrap_or_default()),
                Style::default().yellow().bold(),
            ),
            Span::styled(
                if self.is_running {
                    "Running...".to_owned()
                } else {
                    "Stopped".to_owned()
                },
                if self.is_running {
                    Style::default().light_green()
                } else {
                    Style::default()
                },
            ),
        ]))
        .on_dark_gray()
    }
}

impl Component for RunAction<'_> {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        current_active_area: ActiveArea,
    ) -> io::Result<()> {
        let layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(vec![
                layout::Constraint::Percentage(10),
                layout::Constraint::Percentage(90),
            ])
            .split(area);
        frame.render_widget(self.status_bar(), layout[0]);
        self.text_areas
            .render(frame, layout[1], current_active_area)
    }
}
