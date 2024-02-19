use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::CrosstermBackend;
use ratatui::widgets::TableState;
use ratatui::Terminal;
use std::io;

use crate::db::{db_trait::Db, dto::Project};

use super::{app::App, event::EventHandler};

pub trait StatefulTable {
    fn items_len(&self) -> usize;
    fn table_state(&mut self) -> &mut TableState;
    fn next(&mut self) {
        let items_len = self.items_len();
        let i = match self.table_state().selected() {
            Some(i) => {
                if i >= items_len - 1 {
                    items_len - 1
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state().select(Some(i));
    }
    fn previous(&mut self) {
        let i = match self.table_state().selected() {
            Some(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state().select(Some(i));
    }
}

// main entry point to enter in ui mode
pub async fn run(projects: Vec<Project>, db: Box<dyn Db>) -> anyhow::Result<()> {
    // setup terminal
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut event_handler = EventHandler::new();
    // create app and run it
    // application state
    let mut app = App::new(projects, db);
    // let res = run_app(&mut terminal, event_handler).await;

    loop {
        let event = event_handler.next().await?;
        let should_quit = app.handle_event(&event)?;
        if should_quit {
            break;
        }
        terminal.draw(|f| app.ui(f))?;

        //terminal.draw(|f| app.ui(f))?;
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}
