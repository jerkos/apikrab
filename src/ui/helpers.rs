use ratatui::widgets::{ListState, TableState};

pub trait Selectable {
    fn selected(&self) -> Option<usize>;
    fn select(&mut self, index: Option<usize>);
}

impl Selectable for ListState {
    fn selected(&self) -> Option<usize> {
        return self.selected();
    }

    fn select(&mut self, index: Option<usize>) {
        return self.select(index);
    }
}

pub trait Stateful<T>
where
    T: Selectable,
{
    fn items_len(&self) -> usize;
    fn state(&mut self) -> &mut T;
    fn next(&mut self) {
        let items_len = self.items_len();
        let i = match self.state().selected() {
            Some(i) => {
                if i >= items_len - 1 {
                    items_len - 1
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state().select(Some(i));
    }
    fn previous(&mut self) {
        let i = match self.state().selected() {
            Some(i) => {
                if i == 0 {
                    0
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state().select(Some(i));
    }

    fn unselect(&mut self) {
        self.state().select(None);
    }
}

#[derive(Default, Clone)]
pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> Stateful<ListState> for StatefulList<T> {
    fn items_len(&self) -> usize {
        return self.items.len();
    }

    fn state(&mut self) -> &mut ListState {
        return &mut self.state;
    }
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }
}
