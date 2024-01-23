use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::app::ActiveArea;

pub fn status_bar(active_area: ActiveArea) -> Paragraph<'static> {
    Paragraph::new(
            Line::from(

                vec![
                    if active_area == ActiveArea::ProjectPane {
                        Span::styled(" → action pane   ↓ next project  ↑ previous project", Style::default().fg(Color::DarkGray))
                    } else if active_area == ActiveArea::ActionPane {
                        Span::styled(" ← project pane   ↓ next action  ↑ previous action    Ctrl-b body example    Ctrl-r response example    ESC action pane", Style::default().fg(Color::DarkGray))
                    } else {
                        Span::styled("", Style::default().fg(Color::DarkGray))
                    },
                ]
            )
        ).bg(Color::DarkGray).fg(Color::White)
}
