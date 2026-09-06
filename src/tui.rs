use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
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

#[derive(PartialEq)]
enum Pane {
    Tree,
    Fields,
}

/// One visible row of the tree, in display order.
struct Row {
    path: PathBuf,
    label: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
    procs: Option<usize>,
}

struct App {
    root: PathBuf,
    data: CgroupData,
    expanded: HashSet<PathBuf>,
    selected: usize,
    fields_scroll: u16,
    fields_height: u16,
    fields_lines: usize,
    pane: Pane,
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
            fields_scroll: 0,
            fields_height: 0,
            fields_lines: 0,
            pane: Pane::Tree,
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
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Tab => {
                self.pane = match self.pane {
                    Pane::Tree => Pane::Fields,
                    Pane::Fields => Pane::Tree,
                };
            }
            KeyCode::Char('r') => self.rescan(),
            _ => match self.pane {
                Pane::Tree => self.handle_tree_key(key.code),
                Pane::Fields => self.handle_fields_key(key.code),
            },
        }
        false
    }

    fn handle_tree_key(&mut self, code: KeyCode) {
        let rows = self.rows();
        match code {
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
                    return;
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
    }

    fn handle_fields_key(&mut self, code: KeyCode) {
        let max = (self.fields_lines as u16).saturating_sub(self.fields_height);
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.fields_scroll = (self.fields_scroll + 1).min(max)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.fields_scroll = self.fields_scroll.saturating_sub(1)
            }
            KeyCode::PageDown => {
                self.fields_scroll = (self.fields_scroll + self.fields_height).min(max)
            }
            KeyCode::PageUp => {
                self.fields_scroll = self.fields_scroll.saturating_sub(self.fields_height)
            }
            KeyCode::Home | KeyCode::Char('g') => self.fields_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.fields_scroll = max,
            _ => {}
        }
    }

    fn select(&mut self, index: usize, rows: &[Row]) {
        let index = index.min(rows.len().saturating_sub(1));
        if index != self.selected {
            self.selected = index;
            self.fields_scroll = 0;
        }
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
        self.flatten(&self.data.root, 0, &mut rows);
        rows
    }

    fn flatten(&self, node: &CgroupNode, depth: usize, rows: &mut Vec<Row>) {
        let expanded = self.expanded.contains(&node.path);
        rows.push(Row {
            path: node.path.clone(),
            label: node.name.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded,
            procs: node.procs,
        });
        if expanded {
            for child in &node.children {
                self.flatten(child, depth + 1, rows);
            }
        }
    }

    /// Finds a node by path in the tree.
    fn find_node(&self, path: &Path) -> Option<&CgroupNode> {
        Self::find_in_subtree(&self.data.root, path)
    }

    fn find_in_subtree<'a>(node: &'a CgroupNode, path: &Path) -> Option<&'a CgroupNode> {
        if node.path == path {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = Self::find_in_subtree(child, path) {
                return Some(found);
            }
        }
        None
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [main, footer] =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(main);

        let rows = self.rows();
        self.selected = self.selected.min(rows.len().saturating_sub(1));
        self.draw_tree(frame, left, &rows);
        self.draw_fields(frame, right, &rows);
        self.draw_footer(frame, footer);
    }

    fn pane_block(&self, title: String, pane: Pane) -> Block<'static> {
        let border = if self.pane == pane {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title)
    }

    fn draw_tree(&mut self, frame: &mut Frame, area: Rect, rows: &[Row]) {
        let items: Vec<ListItem> = rows
            .iter()
            .map(|row| {
                let marker = if !row.has_children {
                    "  "
                } else if row.expanded {
                    "▾ "
                } else {
                    "▸ "
                };
                let mut spans = vec![
                    Span::raw("  ".repeat(row.depth)),
                    Span::styled(marker, Style::default().fg(Color::DarkGray)),
                    Span::raw(row.label.clone()),
                ];
                if let Some(n) = row.procs.filter(|&n| n > 0) {
                    spans.push(Span::styled(
                        format!("  ({n})"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();

        let title = format!(" cgroups ({}) ", self.data.count());
        let list = List::new(items)
            .block(self.pane_block(title, Pane::Tree))
            .highlight_style(
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn draw_fields(&mut self, frame: &mut Frame, area: Rect, rows: &[Row]) {
        let Some(row) = rows.get(self.selected) else {
            return;
        };

        // Build lines from the node's pre-loaded fields.
        // We need to do this in a block to drop the borrow before modifying self.
        let mut lines: Vec<Line> = {
            let node = self.find_node(&row.path);
            let mut lines_temp: Vec<Line> = Vec::new();

            if let Some(node) = node {
                for field in &node.fields {
                    let name_style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD);
                    let mut value_lines = field.value.lines();
                    let first = value_lines.next().unwrap_or("").to_string();
                    lines_temp.push(Line::from(vec![
                        Span::styled(field.name.clone(), name_style),
                        Span::raw("  "),
                        Span::raw(first),
                    ]));
                    for extra in value_lines {
                        lines_temp.push(Line::from(format!("  {extra}")));
                    }
                }
            }
            lines_temp
        }; // Borrow of self is dropped here

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "no readable interface files",
                Style::default().fg(Color::DarkGray),
            )));
        }

        self.fields_lines = lines.len();
        self.fields_height = area.height.saturating_sub(2);
        let max = (self.fields_lines as u16).saturating_sub(self.fields_height);
        self.fields_scroll = self.fields_scroll.min(max);

        let rel = row.path.strip_prefix(&self.root).unwrap_or(&row.path);
        let title = if rel.as_os_str().is_empty() {
            " / ".to_string()
        } else {
            format!(" /{} ", rel.display())
        };
        let fields = Paragraph::new(lines)
            .block(self.pane_block(title, Pane::Fields))
            .scroll((self.fields_scroll, 0));
        frame.render_widget(fields, area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help =
            " q quit · ↑↓/jk move · ←→/hl collapse/expand · enter toggle · tab pane · r refresh";
        frame.render_widget(
            Paragraph::new(Span::styled(help, Style::default().fg(Color::DarkGray))),
            area,
        );
    }
}
