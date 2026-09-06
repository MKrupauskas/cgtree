use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::{DefaultTerminal, Frame};

use crate::cgroup;
use crate::data::{CgroupData, CgroupNode};

pub fn run(root: &Path, data: CgroupData) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        bail!("interactive view needs a terminal; use `cgtree list` when piping");
    }
    let mut app = App::new(root.to_path_buf(), data);
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

/// One visible row of the tree, in display order.
struct Row {
    path: PathBuf,
    label: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
    /// Which child index this is among its siblings (for drawing tree lines)
    is_last: bool,
    /// For each depth level, whether we need to draw a vertical line
    ancestor_lines: Vec<bool>,
}

struct App {
    root: PathBuf,
    data: CgroupData,
    expanded: HashSet<PathBuf>,
    selected: usize,
}

impl App {
    fn new(root: PathBuf, data: CgroupData) -> Self {
        let mut expanded = HashSet::new();
        expanded.insert(data.root.path.clone());
        App {
            root,
            data,
            expanded,
            selected: 0,
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && self.handle_key(key)
            {
                return Ok(());
            }
        }
    }

    /// Returns true when the app should quit.
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }

        let rows = self.rows();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('r') => self.rescan(),
            KeyCode::Down | KeyCode::Char('j') => self.select(self.selected + 1, &rows),
            KeyCode::Up | KeyCode::Char('k') => self.select(self.selected.saturating_sub(1), &rows),
            KeyCode::Home | KeyCode::Char('g') => self.select(0, &rows),
            KeyCode::End | KeyCode::Char('G') => self.select(rows.len().saturating_sub(1), &rows),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(row) = rows.get(self.selected)
                    && row.has_children
                    && !self.expanded.remove(&row.path)
                {
                    self.expanded.insert(row.path.clone());
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(row) = rows.get(self.selected)
                    && row.has_children
                {
                    self.expanded.insert(row.path.clone());
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let Some(row) = rows.get(self.selected) else {
                    return false;
                };
                if row.expanded {
                    self.expanded.remove(&row.path);
                } else if let Some(parent) = rows[..self.selected]
                    .iter()
                    .rposition(|r| r.depth < row.depth)
                {
                    self.select(parent, &rows);
                }
            }
            _ => {}
        }
        false
    }

    fn select(&mut self, index: usize, rows: &[Row]) {
        self.selected = index.min(rows.len().saturating_sub(1));
    }

    fn rescan(&mut self) {
        if let Ok(data) = cgroup::scan(&self.root) {
            self.data = data;
            let rows = self.rows();
            self.selected = self.selected.min(rows.len().saturating_sub(1));
        }
    }

    /// Flattens the expanded portion of the tree into display order.
    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.flatten(&self.data.root, 0, &mut rows, Vec::new(), true);
        rows
    }

    fn flatten(
        &self,
        node: &CgroupNode,
        depth: usize,
        rows: &mut Vec<Row>,
        ancestor_lines: Vec<bool>,
        is_last: bool,
    ) {
        let expanded = self.expanded.contains(&node.path);
        let has_children = !node.children.is_empty();

        rows.push(Row {
            path: node.path.clone(),
            label: if has_children {
                format!("{}/", node.name)
            } else {
                node.name.clone()
            },
            depth,
            has_children,
            expanded,
            is_last,
            ancestor_lines: ancestor_lines.clone(),
        });

        if expanded && has_children {
            let new_ancestor_lines = if depth > 0 {
                let mut lines = ancestor_lines.clone();
                lines.push(!is_last);
                lines
            } else {
                // Root's children (depth 1) should have no indentation
                Vec::new()
            };

            for (i, child) in node.children.iter().enumerate() {
                let is_last_child = i == node.children.len() - 1;
                self.flatten(child, depth + 1, rows, new_ancestor_lines.clone(), is_last_child);
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [main, footer] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

        let rows = self.rows();
        self.selected = self.selected.min(rows.len().saturating_sub(1));
        self.draw_tree(frame, main, &rows);
        self.draw_footer(frame, footer);
    }

    fn draw_tree(&mut self, frame: &mut Frame, area: Rect, rows: &[Row]) {
        let items: Vec<ListItem> = rows
            .iter()
            .map(|row| {
                let mut spans = Vec::new();

                // Draw the tree structure matching the list command
                if row.depth == 0 {
                    // Root node - no prefix
                    spans.push(Span::raw(row.label.clone()));
                } else {
                    // Draw ancestor lines
                    for &draw_line in &row.ancestor_lines {
                        if draw_line {
                            spans.push(Span::styled("│   ", Style::default().fg(Color::DarkGray)));
                        } else {
                            spans.push(Span::raw("    "));
                        }
                    }

                    // Draw the branch connector
                    if row.is_last {
                        spans.push(Span::styled("└── ", Style::default().fg(Color::DarkGray)));
                    } else {
                        spans.push(Span::styled("├── ", Style::default().fg(Color::DarkGray)));
                    }

                    spans.push(Span::raw(row.label.clone()));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = " q quit · ↑↓/jk move · ←→/hl collapse/expand · enter/space toggle · r refresh";
        frame.render_widget(
            Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
            area,
        );
    }
}
