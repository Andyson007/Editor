use core::panic;
use std::{
    cmp, io,
    ops::{Index, IndexMut},
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
            normal: {
                let mut trie: Trie<KeyEvent, Action> = Trie::new();
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| {
                        block_on(client.enter_insert(client.curr().cursorpos))
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| {
                        block_on(client.enter_insert(client.curr().cursorpos + (0, 1)))
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| {
                        client.modeinfo.set_mode(Mode::Command(String::new()));
                        Ok(())
                    }),
                );
                for x in [KeyCode::Char('h'), KeyCode::Left] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(move |client: &mut Client| {
                            client.move_left();
                            Ok(())
                        }),
                    );
                }
                for x in [KeyCode::Char('j'), KeyCode::Down] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(move |client: &mut Client| {
                            client.move_down();
                            Ok(())
                        }),
                    );
                }
                for x in [KeyCode::Char('k'), KeyCode::Up] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(move |client: &mut Client| {
                            client.move_up();
                            Ok(())
                        }),
                    );
                }
                for x in [KeyCode::Char('l'), KeyCode::Right] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(move |client: &mut Client| {
                            client.move_right();
                            Ok(())
                        }),
                    );
                }
                trie
            },
            insert: {
                let mut trie: Trie<KeyEvent, Action> = Trie::new();
                for c in (32..255).map(char::from_u32).map(Option::unwrap) {
                    trie.insert(
                        [KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)],
                        Box::new(move |client: &mut Client| block_on(client.type_char(c))),
                    );
                }
                trie.insert(
                    [KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| block_on(client.type_char('\n'))),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| block_on(client.exit_insert())),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| block_on(client.backspace())),
                );
                trie
            },
            command: {
                let mut trie: Trie<KeyEvent, Action> = Trie::new();
                trie.insert(
                    [KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| {
                        let Mode::Command(ref mut x) = client.modeinfo.mode else {
                            unreachable!()
                        };
                        if x.pop().is_none() {
                            client.modeinfo.mode = Mode::Normal;
                        };
                        Ok(())
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)],
                    Box::new(move |client: &mut Client| {
                        let Mode::Command(ref x) = client.modeinfo.mode else {
                            unreachable!()
                        };
                        let x = x.clone();
                        if block_on(client.execute_command(&x))? {
                            return Ok(());
                        }
                        client.modeinfo.set_mode(Mode::Normal);
                        Ok(())
                    }),
                );
                for c in ('a'..='z').chain('A'..='Z') {
                    trie.insert(
                        [KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)],
                        Box::new(move |client: &mut Client| {
                            let Mode::Command(ref mut x) = client.modeinfo.mode else {
                                unreachable!()
                            };
                            x.push(c);
                            Ok(())
                        }),
                    );
                }
                trie
            },
        }
    }
}

impl Index<&Mode> for Bindings {
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
