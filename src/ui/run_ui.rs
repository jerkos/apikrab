use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::Backend;
use ratatui::prelude::CrosstermBackend;
use ratatui::widgets::TableState;
use ratatui::{Frame, Terminal};
use std::io;

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

pub trait UIRunner {
    fn handle_event(&mut self) -> io::Result<bool> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let KeyCode::Char('q') = key.code {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn ui<B: Backend>(&mut self, f: &mut Frame<B>);

    fn run_ui(&mut self) -> anyhow::Result<()> {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // create app and run it
        let res = self.run_app(&mut terminal);

        // restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{err:?}");
        }

        Ok(())
    }

    fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            let should_quit = self.handle_event()?;
            if should_quit {
                return Ok(());
            }
        }
    }
}
