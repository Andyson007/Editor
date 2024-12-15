use std::ops::{Index, IndexMut};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use trie::Trie;

use super::client::{Client, Mode};

pub(crate) struct Bindings {
    insert: Trie<KeyEvent, Box<dyn Fn(&mut Client)>>,
    normal: Trie<KeyEvent, Box<dyn Fn(&mut Client)>>,
    command: Trie<KeyEvent, Box<dyn Fn(&mut Client)>>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            insert: Default::default(),
            normal: {
                let mut trie: Trie<KeyEvent, Box<dyn Fn(&mut Client)>> = Trie::new();
                trie.insert(
                    [
                        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                    ],
                    Box::new(|client: &mut Client| panic!()),
                );
                trie
            },
            command: Default::default(),
        }
    }
}

impl Index<&Mode> for Bindings {
    type Output = Trie<KeyEvent, Box<dyn Fn(&mut Client)>>;

    fn index(&self, mode: &Mode) -> &Self::Output {
        match mode {
            Mode::Normal => &self.normal,
            Mode::Insert => &self.insert,
            Mode::Command(_) => &self.command,
        }
    }
}

impl IndexMut<&Mode> for Bindings {
    fn index_mut(&mut self, mode: &Mode) -> &mut Self::Output {
        match mode {
            Mode::Normal => &mut self.normal,
            Mode::Insert => &mut self.insert,
            Mode::Command(_) => &mut self.command,
        }
    }
}
