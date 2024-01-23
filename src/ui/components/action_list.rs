use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    commands::run::_run_helper::ANONYMOUS_ACTION,
    db::dto::Action,
    ui::{
        app::ActiveArea,
        helpers::{highlight_if_needed, Component, StatefulList},
    },
    utils::human_readable_date,
};

pub struct ActionList<'a> {
    pub actions: &'a mut StatefulList<Action>,
}

impl<'a> Component for ActionList<'a> {
    fn render<B: ratatui::prelude::Backend>(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: Rect,
        current_active_area: ActiveArea,
    ) -> std::io::Result<()> {
        let action_list = List::new(
            self.actions
                .items
                .iter()
                .map(|a| {
                    let first_domain_action = &a.actions[0];
                    let verb: String = first_domain_action.verb.clone();
                    let url = first_domain_action
                        .urls
                        .iter()
                        .next()
                        .cloned()
                        .unwrap_or_default();
                    let action_name = a.name.clone().unwrap_or(ANONYMOUS_ACTION.to_owned());
                    ListItem::new(vec![
                        Line::styled(action_name, Style::default().fg(Color::LightGreen).bold()),
                        Line::from(vec![
                            Span::raw("    "),
                            match verb.as_str() {
                                "POST" => Span::styled(
                                    verb,
                                    Style::default().fg(Color::DarkGray).bg(Color::Green),
                                ),
                                "GET" => Span::styled(
                                    verb,
                                    Style::default().fg(Color::DarkGray).bg(Color::Blue),
                                ),
                                "DELETE" => Span::styled(
                                    verb,
                                    Style::default().fg(Color::DarkGray).bg(Color::Red),
                                ),
                                "PUT" => Span::styled(
                                    verb,
                                    Style::default()
                                        .fg(Color::DarkGray)
                                        // purple
                                        .bg(Color::Rgb(128, 0, 128)),
                                ),
                                "PATCH" => Span::styled(
                                    verb,
                                    // purple
                                    Style::default()
                                        .fg(Color::DarkGray)
                                        .bg(Color::Rgb(255, 128, 0)),
                                ),
                                _ => Span::styled(
                                    verb,
                                    Style::default().fg(Color::DarkGray).bg(Color::Yellow),
                                ),
                            },
                            Span::raw(" "),
                            Span::styled(url, Style::default().fg(Color::LightBlue)),
                            Span::raw(" "),
                            Span::styled(
                                if first_domain_action.body.1 {
                                    "(form)"
                                } else if first_domain_action.body.2 {
                                    "(url encoded)"
                                } else {
                                    "(json)"
                                },
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                a.created_at
                                    .as_ref()
                                    .map(human_readable_date)
                                    .unwrap_or("".to_string()),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                a.updated_at
                                    .as_ref()
                                    .map(human_readable_date)
                                    .unwrap_or("".to_string()),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]),
                    ])
                })
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title("Actions".gray())
                .style(Style::reset())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(highlight_if_needed(
                    current_active_area,
                    ActiveArea::ActionPane,
                ))),
        )
        .highlight_style(Style::default().fg(Color::White))
        .highlight_symbol(">>");

        frame.render_stateful_widget(action_list, area, &mut self.actions.state);

        Ok(())
    }
}
