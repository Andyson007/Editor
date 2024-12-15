use std::{
    future::Future,
    io,
    ops::{Index, IndexMut},
    pin::Pin,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::executor::block_on;
use trie::Trie;

use super::client::{Client, Mode};

type Action = Box<dyn Fn(&mut Client) -> io::Result<()>>;

pub(crate) struct Bindings {
    insert: Trie<KeyEvent, Action>,
    normal: Trie<KeyEvent, Action>,
    command: Trie<KeyEvent, Action>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            normal: Default::default(),
            insert: {
                let mut trie: Trie<KeyEvent, Action> = Trie::new();
                trie.insert(
                    [
                        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
                        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
                    ],
                    Box::new(|client: &mut Client| {
                        block_on(client.handle_insert_keyevent(KeyEvent::new(
                            KeyCode::Esc,
                            KeyModifiers::NONE,
                        )))
                    }),
                );
                trie
            },
            command: Default::default(),
        }
    }
}

impl<'a> Index<&Mode> for Bindings {
    type Output = Trie<KeyEvent, Action>;

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
