//! A headless driver for the cgtree explorer.
//!
//! Tests here are end-to-end from the user's point of view: they build a real
//! cgroup v2 hierarchy on disk, scan it with the same code the binary uses,
//! send real key events, and assert on what a terminal would actually show.
//! Nothing is stubbed — only the terminal itself is swapped for ratatui's
//! in-memory `TestBackend`.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use cgtree::tui::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use tempfile::TempDir;

/// A synthetic cgroup v2 hierarchy on disk.
pub struct Fixture {
    tempdir: TempDir,
}

impl Fixture {
    pub fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let f = Self { tempdir };
        // The root of a v2 hierarchy always has cgroup.controllers.
        f.add("", &[]);
        f
    }

    pub fn root(&self) -> &Path {
        self.tempdir.path()
    }

    /// Creates a cgroup at `rel_path` with the given extra interface files.
    pub fn add(&self, rel_path: &str, fields: &[(&str, &str)]) -> &Self {
        let dir = if rel_path.is_empty() {
            self.tempdir.path().to_path_buf()
        } else {
            self.tempdir.path().join(rel_path)
        };
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        fs::write(dir.join("cgroup.procs"), "1000\n").unwrap();
        for (name, value) in fields {
            fs::write(dir.join(name), format!("{value}\n")).unwrap();
        }
        self
    }

    /// Opens the explorer on this hierarchy at the given terminal size.
    pub fn explore(&self, width: u16, height: u16) -> Tui {
        let data = cgtree::cgroup::scan(self.root()).unwrap();
        let app = App::new(self.root().to_path_buf(), data);
        let terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut tui = Tui {
            app,
            terminal,
            root: self.root().to_path_buf(),
            quit: false,
        };
        tui.render();
        tui
    }
}

/// A running explorer backed by an in-memory terminal.
pub struct Tui {
    app: App,
    terminal: Terminal<TestBackend>,
    root: PathBuf,
    quit: bool,
}

impl Tui {
    fn render(&mut self) {
        let app = &mut self.app;
        self.terminal.draw(|frame| app.draw(frame)).unwrap();
    }

    /// Sends one key press, then redraws — exactly what the real event loop does.
    pub fn key(&mut self, code: KeyCode) -> &mut Self {
        self.key_with(code, KeyModifiers::NONE)
    }

    pub fn key_with(&mut self, code: KeyCode, modifiers: KeyModifiers) -> &mut Self {
        let event = KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        self.quit = self.app.handle_key(event);
        // The real loop stops drawing once handle_key signals quit.
        if !self.quit {
            self.render();
        }
        self
    }

    /// Sends a character key.
    pub fn press(&mut self, c: char) -> &mut Self {
        self.key(KeyCode::Char(c))
    }

    /// Types a string one character at a time (for the filter input).
    pub fn type_str(&mut self, text: &str) -> &mut Self {
        for c in text.chars() {
            self.press(c);
        }
        self
    }

    /// True once the app has signalled it should exit.
    pub fn has_quit(&self) -> bool {
        self.quit
    }

    /// The visible screen as trimmed lines, with the volatile tempdir path in
    /// the root row replaced by `<root>` so assertions stay stable.
    pub fn screen(&self) -> Vec<String> {
        let buffer = self.terminal.backend().buffer();
        let root = self.root.display().to_string();
        (0..buffer.area.height)
            .map(|y| {
                let line: String = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                line.trim_end().replace(&root, "<root>")
            })
            .collect()
    }

    /// The screen with trailing blank lines dropped.
    pub fn visible(&self) -> Vec<String> {
        let mut lines = self.screen();
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines
    }

    /// The whole screen as one string, for `contains` checks.
    pub fn text(&self) -> String {
        self.screen().join("\n")
    }

    /// The tree rows only — everything above the footer help line.
    pub fn tree(&self) -> Vec<String> {
        self.screen()
            .into_iter()
            .take_while(|l| !l.trim_start().starts_with("? help"))
            .filter(|l| !l.is_empty())
            .collect()
    }

    /// The footer status line.
    pub fn footer(&self) -> String {
        self.screen()
            .into_iter()
            .find(|l| l.trim_start().starts_with("? help"))
            .unwrap_or_default()
    }

    /// The text of the currently highlighted row, identified by its
    /// selection background — the same cue a user sees on screen.
    pub fn selected(&self) -> String {
        let buffer = self.terminal.backend().buffer();
        let root = self.root.display().to_string();
        for y in 0..buffer.area.height {
            if buffer[(0, y)].bg == ratatui::style::Color::Cyan {
                let line: String = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                return line.trim_end().replace(&root, "<root>");
            }
        }
        String::new()
    }

    /// Index of the highlighted row within the visible tree rows.
    pub fn selected_index(&self) -> Option<usize> {
        let buffer = self.terminal.backend().buffer();
        (0..buffer.area.height)
            .position(|y| buffer[(0, y)].bg == ratatui::style::Color::Cyan)
    }
}
