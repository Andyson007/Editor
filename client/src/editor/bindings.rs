use std::{
    cmp, io,
    ops::{Index, IndexMut},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::executor::block_on;
use trie::Trie;
use utils::other::CursorPos;

use super::{
    buffer::{Buffer, BufferData, BufferTypeData},
    client::{Client, Mode},
};

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
                    Box::new(|client: &mut Client| {
                        block_on(client.enter_insert(client.curr().cursorpos))
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('I'), KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        client.curr_mut().cursorpos.col = 0;
                        block_on(client.enter_insert(client.curr().cursorpos))
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        block_on(async {
                            if !client.curr().data.modifiable {
                                return Ok(());
                            }
                            let BufferTypeData::Regular { text, .. } =
                                &client.curr().data.buffer_type
                            else {
                                todo!("You can only type in regular buffers")
                            };
                            client.curr_mut().cursorpos.col = cmp::min(
                                text.lines().nth(client.curr().cursorpos.row).unwrap().len(),
                                client.curr().cursorpos.col + 1,
                            );
                            client.enter_insert(client.curr().cursorpos).await?;
                            Ok(())
                        })
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        block_on(async {
                            if !client.curr().data.modifiable {
                                return Ok(());
                            }
                            let BufferTypeData::Regular { text, .. } =
                                &client.curr().data.buffer_type
                            else {
                                todo!("You can only type in regular buffers")
                            };
                            client.curr_mut().cursorpos.col =
                                text.lines().nth(client.curr().cursorpos.row).unwrap().len();
                            client.enter_insert(client.curr().cursorpos).await?;
                            Ok(())
                        })
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        block_on(async {
                            if !client.curr().data.modifiable {
                                return Ok(());
                            }
                            let BufferTypeData::Regular { text, .. } =
                                &client.curr().data.buffer_type
                            else {
                                todo!("You can only type in regular buffers")
                            };
                            let pos = CursorPos {
                                row: client.curr().cursorpos.row,
                                col: text
                                    .lines()
                                    .nth(client.curr_mut().cursorpos.row)
                                    .map_or(0, |x| x.chars().count()),
                            };
                            client.enter_insert(pos).await?;
                            client.type_char('\n').await?;
                            Ok(())
                        })
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        client.modeinfo.set_mode(Mode::Command(String::new()));
                        Ok(())
                    }),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)],
                    Box::new(|_| Ok(())),
                );
                for x in [KeyCode::Char('h'), KeyCode::Left] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(|client: &mut Client| {
                            client.move_left();
                            Ok(())
                        }),
                    );
                }
                for x in [KeyCode::Char('j'), KeyCode::Down] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(|client: &mut Client| {
                            client.move_down();
                            Ok(())
                        }),
                    );
                }
                for x in [KeyCode::Char('k'), KeyCode::Up] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(|client: &mut Client| {
                            client.move_up();
                            Ok(())
                        }),
                    );
                }
                for x in [KeyCode::Char('l'), KeyCode::Right] {
                    trie.insert(
                        [KeyEvent::new(x, KeyModifiers::NONE)],
                        Box::new(|client: &mut Client| {
                            client.move_right();
                            Ok(())
                        }),
                    );
                }
                trie.insert(
                    [KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        let Buffer {
                            data:
                                BufferData {
                                    buffer_type: BufferTypeData::Folder { inhabitants },
                                    ..
                                },
                            cursorpos: CursorPos { row, .. },
                            path,
                            ..
                        } = client.curr()
                        else {
                            return Ok(());
                        };
                        *client.curr_mut() = block_on(async {
                            Buffer::connect(
                                client.server_addr,
                                &client.username.clone(),
                                #[cfg(feature = "security")]
                                client.password.clone(),
                                &client.color,
                                path.as_ref().unwrap().join(inhabitants[*row].name.clone()),
                            )
                            .await
                        })?;

                        Ok(())
                    }),
                );
                trie
            },
            insert: {
                let mut trie: Trie<KeyEvent, Action> = Trie::new();
                trie.insert(
                    [KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| block_on(client.type_char('\n'))),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| block_on(client.exit_insert())),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
                        block_on(async {
                            client.backspace().await?;
                            Ok(())
                        })
                    }),
                );
                trie.insert(
                    [
                        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
                        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
                    ],
                    Box::new(|client: &mut Client| block_on(client.exit_insert())),
                );
                trie.insert(
                    [KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)],
                    Box::new(|client: &mut Client| {
                        block_on(async {
                            let Some(first_del) = client.backspace().await? else {
                                return Ok(());
                            };

                            if first_del == '\n' {
                                return Ok(());
                            } else if first_del == ' ' {
                                while client.backspace().await?.is_some_and(|x| x == ' ') {}
                            }

                            while let Some(deleted) = client.backspace().await? {
                                if deleted.is_whitespace() {
                                    client.type_char(deleted).await?;
                                    break;
                                }
                            }

                            Ok(())
                        })
                    }),
                );
                trie
            },
            command: {
                let mut trie: Trie<KeyEvent, Action> = Trie::new();
                trie.insert(
                    [KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)],
                    Box::new(|client: &mut Client| {
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
                    Box::new(|client: &mut Client| {
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
