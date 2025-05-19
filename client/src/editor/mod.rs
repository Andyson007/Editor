//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them

use std::{io, net::SocketAddrV4, path::Path, time::Duration};

use bindings::Bindings;
use buffer::Buffer;
use client::{Client, ModeInfo};
use crossterm::{
    event::{KeyCode, KeyEvent},
    style::Color,
};
use text::Text;
use tokio::{io::AsyncWriteExt, net::TcpStream, time};
mod bindings;
mod buffer;
mod client;
mod draw;

pub static BUFFER_SIZE: usize = 8192;

pub struct App {
    pub client: Client,
    pub(crate) bindings: Bindings,
}

impl App {
    pub async fn new(
        username: String,
        #[cfg(feature = "security")] password: &str,
        address: SocketAddrV4,
        color: &Color,
        path: &Path,
    ) -> io::Result<Self> {
        Ok(Self {
            client: Client::from_path(
                username,
                #[cfg(feature = "security")]
                password.to_owned(),
                address,
                color,
                path,
            )
            .await?,
            bindings: Bindings::default(),
        })
    }

    pub fn new_with_buffer(
        username: String,
        #[cfg(feature = "security")] password: String,
        text: Text,
        colors: Vec<Color>,
        socket: Option<TcpStream>,
        address: SocketAddrV4,
        color: &Color,
        path: &Path,
    ) -> Self {
        Self {
            client: {
                let buf = Buffer::new(&username, text, colors, socket, Some(path));
                Client {
                    buffers: Vec::from([buf]),
                    current_buffer: 0,
                    modeinfo: ModeInfo::default(),
                    info: Some("Prewss Escape then :help to view help".to_string()),
                    #[cfg(feature = "security")]
                    password,
                    username,
                    color: *color,
                    server_addr: address,
                }
            },
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
        // I know the array isn't empty
        let fallback = self.client.modeinfo.keymap.remove(0);
        self.handle_fallback(fallback).await
    }

    async fn handle_fallback(&mut self, ev: KeyEvent) -> io::Result<bool> {
        Ok(match self.client.modeinfo.mode {
            client::Mode::Normal => false,
            client::Mode::Insert => match ev.code {
                KeyCode::Char(c) => {
                    self.client.type_char(c).await?;
                    true
                }
                _ => false,
            },
            client::Mode::Command(_) => false,
        })
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
