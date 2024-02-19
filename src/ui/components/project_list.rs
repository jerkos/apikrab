use std::collections::HashMap;

use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    db::dto::Project,
    ui::{
        app::ActiveArea,
        helpers::{highlight_if_needed, Component, StatefulList},
    },
};

pub struct ProjectList<'a> {
    pub projects: &'a mut StatefulList<Project>,
}

impl<'a> Component for ProjectList<'a> {
    fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: Rect,
        current_active_area: ActiveArea,
    ) -> std::io::Result<()> {
        let project_list = List::new(
            self.projects
                .items
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let mut conf_keys = p
                        .get_project_conf()
                        .unwrap_or(HashMap::new())
                        .keys()
                        .map(String::to_string)
                        .collect::<Vec<_>>();
                    conf_keys.sort();
                    ListItem::new(vec![Line::styled(
                        format!(" {}. {}({})", i, p.name.clone(), conf_keys.join(", ")),
                        Style::default().fg(Color::LightGreen).bold(),
                    )])
                })
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title("Projects".gray())
                .style(Style::reset())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(highlight_if_needed(
                    current_active_area,
                    ActiveArea::ProjectPane,
                ))),
        )
        .highlight_style(Style::default().fg(Color::Yellow))
        .highlight_symbol(">>");
        frame.render_stateful_widget(project_list, area, &mut self.projects.state);
        Ok(())
    }
}
