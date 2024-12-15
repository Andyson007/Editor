//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them

use std::{io, mem, time::Duration};

use bindings::Bindings;
use client::{Client, Mode};
use crossterm::{event::KeyEvent, style::Color};
use text::Text;
use tokio::{io::AsyncWriteExt, net::TcpStream, time};
mod bindings;
mod buffer;
mod client;
mod draw;

pub struct App {
    pub client: Client,
    pub(crate) bindings: Bindings,
}

impl App {
    pub fn new(username: String) -> Self {
        Self {
            client: Client::new(username),
            bindings: Bindings::default(),
        }
    }

    pub fn new_with_buffer(
        username: String,
        text: Text,
        colors: Vec<Color>,
        socket: Option<TcpStream>,
    ) -> Self {
        Self {
            client: Client::new_with_buffer(username, text, colors, socket),
            bindings: Bindings::default(),
        }
    }

    pub async fn execute_keyevents(&mut self) -> io::Result<bool> {
        let keymap = mem::take(&mut self.client.modeinfo.keymap);

        let mode = self.client.modeinfo.mode.clone();
        let binding = self.bindings[&mode].get(keymap.iter().copied());

        let Some((node, _)) = binding else {
            for key in &keymap {
                match self.client.modeinfo.mode {
                    Mode::Normal => self.client.handle_normal_keyevent(key).await?,
                    Mode::Insert => self.client.handle_insert_keyevent(key).await?,
                    Mode::Command(_) => {
                        if self.client.handle_command_keyevent(key).await? {
                            return Ok(true);
                        }
                    }
                };
            }
            return Ok(false);
        };
        node(&mut self.client);
        if let Some(buffer::Socket { ref mut writer, .. }) = self.client.curr_mut().socket {
            writer.flush().await?;
        }
        Ok(false)
    }

    /// Handles a keyevent. This method handles every `mode`
    pub async fn handle_keyevent(&mut self, input: &KeyEvent) -> io::Result<()> {
        self.client.modeinfo.keymap.push(*input);
        if self.bindings[&self.client.modeinfo.mode]
            .exists_child(self.client.modeinfo.keymap.iter().copied())
        {
            self.client.modeinfo.timer = Some(time::sleep(Duration::from_secs(1)));
        } else {
            self.client.modeinfo.timer = Some(time::sleep(Duration::ZERO));
        }
        Ok(())
    }
}
