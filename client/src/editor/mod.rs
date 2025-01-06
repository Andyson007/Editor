//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them

use std::{io, time::Duration};

use bindings::Bindings;
use client::Client;
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
    pub async fn new(username: String, socket: TcpStream) -> io::Result<Self> {
        Ok(Self {
            client: Client::from_socket(username, socket).await?,
            bindings: Bindings::default(),
        })
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
        self.client.modeinfo.timer = None;
        while !self.client.modeinfo.keymap.is_empty() {
            self.execute_top_keyevent().await?;
        }
        if let Some(buffer::Socket { ref mut writer, .. }) = self.client.curr_mut().socket {
            writer.flush().await?;
        }
        Ok(false)
    }

    /// executes the longest command from the current keymap
    /// # Note
    /// does not flush the socket
    async fn execute_top_keyevent(&mut self) -> io::Result<bool> {
        if self.client.modeinfo.keymap.is_empty() {
            return Ok(false);
        }
        let modeinfo = &self.client.modeinfo;
        for i in (1..=self.client.modeinfo.keymap.len()).rev() {
            let binding = self.bindings[&modeinfo.mode].get(modeinfo.keymap[0..i].iter().copied());
            if let Some((node, _)) = binding {
                node(&mut self.client)?;
                self.client.modeinfo.keymap.drain(0..i);
                return Ok(true);
            };
        }
        panic!(
            "There aren't any keybinds available for {:?} when in {:?} mode",
            self.client.modeinfo.keymap, self.client.modeinfo.mode
        );
    }

    pub async fn handle_keyevent(&mut self, input: &KeyEvent) -> io::Result<bool> {
        self.client.modeinfo.keymap.push(*input);
        let mut should_flush = false;
        while !self.bindings[&self.client.modeinfo.mode]
            .exists_child(self.client.modeinfo.keymap.iter().copied())
        {
            should_flush = self.execute_top_keyevent().await?;
        }
        if should_flush {
            if let Some(buffer::Socket { ref mut writer, .. }) = self.client.curr_mut().socket {
                writer.flush().await?;
            }
        }
        if !self.client.modeinfo.keymap.is_empty() {
            self.client.modeinfo.timer = Some(time::sleep(Duration::from_secs(1)));
        }
        Ok(should_flush)
    }
}
