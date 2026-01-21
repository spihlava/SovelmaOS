//! Command-line shell with input handling.
//!
//! Provides line editing and command history.

use super::commands::Command;
use crate::arch::x86_64::vga::{self, Color};
use crate::{print, println};
use alloc::string::String;
use alloc::vec::Vec;
use pc_keyboard::DecodedKey;

/// Maximum input line length.
const MAX_LINE_LENGTH: usize = 256;

/// Maximum command history size.
const MAX_HISTORY: usize = 16;

/// Terminal shell with line editing and history.
pub struct Terminal {
    /// Current input buffer.
    input_buffer: String,
    /// Cursor position in input buffer.
    cursor: usize,
    /// Command history.
    history: Vec<String>,
    /// Current position in history (for up/down navigation).
    history_index: Option<usize>,
    /// Saved input when navigating history.
    saved_input: String,
}

impl Terminal {
    /// Create a new terminal.
    pub fn new() -> Self {
        Self {
            input_buffer: String::with_capacity(MAX_LINE_LENGTH),
            cursor: 0,
            history: Vec::with_capacity(MAX_HISTORY),
            history_index: None,
            saved_input: String::new(),
        }
    }

    /// Display the shell prompt.
    pub fn prompt(&self) {
        vga::set_color(Color::LightGreen, Color::Black);
        print!("sovelma");
        vga::set_color(Color::White, Color::Black);
        print!("> ");
    }

    /// Handle a decoded key input.
    ///
    /// Returns a command if the user pressed Enter with a valid command.
    pub fn handle_key(&mut self, key: DecodedKey) -> Option<Command> {
        match key {
            DecodedKey::Unicode(c) => self.handle_char(c),
            DecodedKey::RawKey(raw) => {
                self.handle_raw_key(raw);
                None
            }
        }
    }

    /// Handle a Unicode character input.
    fn handle_char(&mut self, c: char) -> Option<Command> {
        match c {
            '\n' | '\r' => {
                println!(); // Move to next line
                let command = self.parse_command();

                // Add to history if not empty
                if !self.input_buffer.is_empty() {
                    self.add_to_history(self.input_buffer.clone());
                }

                self.input_buffer.clear();
                self.cursor = 0;
                self.history_index = None;

                if command.is_some() {
                    return command;
                }

                // Show prompt for next command
                self.prompt();
                None
            }
            '\x08' | '\x7f' => {
                // Backspace
                if self.cursor > 0 {
                    self.input_buffer.remove(self.cursor - 1);
                    self.cursor -= 1;
                    self.redraw_line();
                }
                None
            }
            '\t' => {
                // Tab - could implement auto-completion here
                None
            }
            c if c.is_ascii() && !c.is_control() => {
                if self.input_buffer.len() < MAX_LINE_LENGTH {
                    self.input_buffer.insert(self.cursor, c);
                    self.cursor += 1;

                    // If cursor is at end, just print the char
                    if self.cursor == self.input_buffer.len() {
                        print!("{}", c);
                    } else {
                        self.redraw_line();
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Handle a raw (non-Unicode) key.
    fn handle_raw_key(&mut self, key: pc_keyboard::KeyCode) {
        use pc_keyboard::KeyCode;

        match key {
            KeyCode::ArrowUp => {
                self.history_up();
            }
            KeyCode::ArrowDown => {
                self.history_down();
            }
            KeyCode::ArrowLeft => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    print!("\x1b[D"); // Move cursor left
                }
            }
            KeyCode::ArrowRight => {
                if self.cursor < self.input_buffer.len() {
                    self.cursor += 1;
                    print!("\x1b[C"); // Move cursor right
                }
            }
            KeyCode::Home => {
                self.cursor = 0;
                self.redraw_line();
            }
            KeyCode::End => {
                self.cursor = self.input_buffer.len();
                self.redraw_line();
            }
            KeyCode::Delete => {
                if self.cursor < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor);
                    self.redraw_line();
                }
            }
            _ => {}
        }
    }

    /// Navigate up in command history.
    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Save current input and go to most recent history
                self.saved_input = self.input_buffer.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => {
                // Already at oldest entry
                return;
            }
            Some(idx) => {
                self.history_index = Some(idx - 1);
            }
        }

        if let Some(idx) = self.history_index {
            self.input_buffer = self.history[idx].clone();
            self.cursor = self.input_buffer.len();
            self.redraw_line();
        }
    }

    /// Navigate down in command history.
    fn history_down(&mut self) {
        match self.history_index {
            None => {
                // Not in history mode
            }
            Some(idx) if idx + 1 >= self.history.len() => {
                // Return to saved input
                self.history_index = None;
                self.input_buffer = self.saved_input.clone();
                self.cursor = self.input_buffer.len();
                self.redraw_line();
            }
            Some(idx) => {
                self.history_index = Some(idx + 1);
                self.input_buffer = self.history[idx + 1].clone();
                self.cursor = self.input_buffer.len();
                self.redraw_line();
            }
        }
    }

    /// Add a command to history.
    fn add_to_history(&mut self, cmd: String) {
        // Don't add duplicates of the last command
        if self.history.last() == Some(&cmd) {
            return;
        }

        if self.history.len() >= MAX_HISTORY {
            self.history.remove(0);
        }
        self.history.push(cmd);
    }

    /// Redraw the current input line.
    fn redraw_line(&self) {
        // Move to start of line, clear it, print prompt and input
        print!("\r");
        vga::set_color(Color::LightGreen, Color::Black);
        print!("sovelma");
        vga::set_color(Color::White, Color::Black);
        print!("> {}", self.input_buffer);

        // Clear any remaining characters from previous line
        print!("  \r");

        // Reprint and position cursor
        vga::set_color(Color::LightGreen, Color::Black);
        print!("sovelma");
        vga::set_color(Color::White, Color::Black);
        print!("> {}", self.input_buffer);
    }

    /// Parse the current input buffer into a command.
    fn parse_command(&self) -> Option<Command> {
        let input = self.input_buffer.trim();
        if input.is_empty() {
            return None;
        }

        let mut parts = input.split_whitespace();
        let cmd = parts.next()?;
        let args: Vec<&str> = parts.collect();

        Command::parse(cmd, &args)
    }

    /// Get the current input buffer.
    pub fn input(&self) -> &str {
        &self.input_buffer
    }

    /// Clear the terminal screen.
    pub fn clear(&self) {
        vga::clear_screen();
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}
