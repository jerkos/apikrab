use crate::{
    db::dto::Action,
    ui::{
        app::ActiveArea,
        custom_renderer::{Renderer, Viewport},
        helpers::{highlight_if_needed, payload_as_str_pretty},
    },
};

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders},
};

#[derive(Clone)]
pub struct TextArea<'a> {
    pub(crate) text_area: tui_textarea::TextArea<'a>,
}

impl<'a> TextArea<'a> {
    pub fn new(tui_text_area: tui_textarea::TextArea<'a>) -> Self {
        Self {
            text_area: tui_text_area,
        }
    }

    // clearing a text area
    fn clear_text_area(&mut self) {
        let text_area = &mut self.text_area;
        text_area.move_cursor(tui_textarea::CursorMove::Top);
        text_area.move_cursor(tui_textarea::CursorMove::Head);

        //for _ in 0..1000 {
        //    text_area.delete_newline();
        text_area.delete_str(100000);
        //}
    }

    fn set_text_inner(&mut self, text: &str) {
        self.text_area.insert_str(text);
        self.text_area.move_cursor(tui_textarea::CursorMove::Top)
    }

    pub fn get_text_area_mut(&mut self) -> &mut tui_textarea::TextArea<'a> {
        &mut self.text_area
    }

    pub fn get_text_area(&self) -> &tui_textarea::TextArea<'a> {
        &self.text_area
    }
}

fn text_area(name: &str) -> TextArea<'_> {
    let mut tui_text_area = tui_textarea::TextArea::default();
    tui_text_area.set_line_number_style(Style::default().bg(Color::DarkGray));
    tui_text_area.set_block(
        Block::default()
            .title(name)
            .style(Style::reset())
            .borders(Borders::ALL),
    );
    TextArea::new(tui_text_area)
}

pub trait DisplayFromAction: Send + Sync {
    fn set_left_text_area_text(&self, action: &Action, text_area: &mut TextArea<'_>);
    fn set_right_text_area_text(&self, action: &Action, text_area: &mut TextArea<'_>);
    fn get_left_active_area(&self) -> ActiveArea;
    fn get_right_active_area(&self) -> ActiveArea;
}

#[derive(Clone)]
pub struct ActionTextAreas<'a> {
    pub action: Option<Action>,
    pub left_text_area: TextArea<'a>,
    pub l_viewport: Viewport,
    pub right_text_area: TextArea<'a>,
    pub r_viewport: Viewport,
    pub clear_text_areas: bool,
    pub displayer: &'a Box<dyn DisplayFromAction>,
}

impl<'a> ActionTextAreas<'a> {
    pub fn new(
        left_text_area_name: &'a str,
        right_text_area_name: &'a str,
        displayer: &'a Box<dyn DisplayFromAction>,
    ) -> Self {
        Self {
            action: None,
            left_text_area: text_area(left_text_area_name),
            l_viewport: Viewport::default(),
            right_text_area: text_area(right_text_area_name),
            r_viewport: Viewport::default(),
            clear_text_areas: false,
            displayer,
        }
    }

    pub fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: Rect,
        current_active_area: ActiveArea,
    ) -> std::io::Result<()> {
        // create layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area);

        // set new text areas if needed
        if self.clear_text_areas {
            let action = self.action.take().unwrap();
            self.displayer
                .set_left_text_area_text(&action, &mut self.left_text_area);
            self.displayer
                .set_right_text_area_text(&action, &mut self.right_text_area);
            self.clear_text_areas = false;
        }

        let r = self.right_text_area.get_text_area_mut();
        r.set_block(r.block().unwrap().clone().border_style(Style::default().fg(
            highlight_if_needed(current_active_area, self.displayer.get_right_active_area()),
        )));

        let b = self.left_text_area.get_text_area_mut();
        b.set_block(b.block().unwrap().clone().border_style(Style::default().fg(
            highlight_if_needed(current_active_area, self.displayer.get_left_active_area()),
        )));

        let left_renderer = Renderer {
            text_area: &self.left_text_area,
            viewport: &self.l_viewport,
        };

        let right_renderer = Renderer {
            text_area: &self.right_text_area,
            viewport: &self.r_viewport,
        };

        frame.render_widget(left_renderer, chunks[0]);
        frame.render_widget(right_renderer, chunks[1]);
        Ok(())
    }
}

pub struct Examples {}

impl DisplayFromAction for Examples {
    fn set_left_text_area_text(&self, action: &Action, left_text_area: &mut TextArea<'_>) {
        let body_ex = payload_as_str_pretty(action.body_example.as_ref()).unwrap();
        left_text_area.clear_text_area();
        left_text_area.set_text_inner(&body_ex);
    }

    fn set_right_text_area_text(&self, action: &Action, right_text_area: &mut TextArea<'_>) {
        let resp_ex = payload_as_str_pretty(action.response_example.as_ref()).unwrap();
        right_text_area.clear_text_area();
        right_text_area.set_text_inner(&resp_ex);
    }

    fn get_left_active_area(&self) -> ActiveArea {
        ActiveArea::BodyExample
    }

    fn get_right_active_area(&self) -> ActiveArea {
        ActiveArea::ResponseExample
    }
}

pub struct DomainActions {}
impl DisplayFromAction for DomainActions {
    fn set_left_text_area_text(&self, action: &Action, text_area: &mut TextArea<'_>) {
        let value = serde_json::to_string_pretty(&action.actions).unwrap_or("".to_string());
        text_area.clear_text_area();
        text_area.set_text_inner(&value);
    }

    fn set_right_text_area_text(&self, _action: &Action, text_area: &mut TextArea<'_>) {
        text_area.clear_text_area();
        text_area.set_text_inner("");
    }

    fn get_left_active_area(&self) -> ActiveArea {
        ActiveArea::DomainAction
    }

    fn get_right_active_area(&self) -> ActiveArea {
        ActiveArea::Result
    }
}
