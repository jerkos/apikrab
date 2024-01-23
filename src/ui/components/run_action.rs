use std::io;

use ratatui::{backend::Backend, layout::Rect, Frame};

use crate::ui::{app::ActiveArea, helpers::Component};

use super::action_text_areas::ActionTextAreas;

#[derive(Clone)]
pub struct RunAction<'a> {
    pub text_areas: ActionTextAreas<'a>,
}

impl Component for RunAction<'_> {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        current_active_area: ActiveArea,
    ) -> io::Result<()> {
        self.text_areas.render::<B>(frame, area, current_active_area)
    }
}
