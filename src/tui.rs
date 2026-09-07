use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::cgroup;
use crate::data::{CgroupData, CgroupNode};
use crate::filter;

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

/// Represents a display item in the TUI list
#[derive(Clone)]
struct DisplayItem {
    /// The visual line content
    line: Line<'static>,
    /// If this is a cgroup row, contains the row index
    cgroup_row_index: Option<usize>,
    /// The row index of the parent cgroup (for field lines, this is the cgroup they belong to)
    parent_cgroup_row_index: usize,
}

enum InputMode {
    Normal,
    Filter,
}

struct App {
    root: PathBuf,
    data: CgroupData,
    expanded: HashSet<PathBuf>,
    /// Index of the currently selected display item (includes both cgroup rows and field lines)
    selected: usize,
    input_mode: InputMode,
    filter_input: Input,
    field_patterns: Vec<String>,
    /// Flat list of all nodes for indexing
    nodes: Vec<PathBuf>,
}

impl App {
    fn new(root: PathBuf, data: CgroupData) -> Self {
        let mut expanded = HashSet::new();
        expanded.insert(data.root.path.clone());
        let mut nodes = Vec::new();
        Self::collect_node_paths(&data.root, &mut nodes);

        App {
            root,
            data,
            expanded,
            selected: 0,
            input_mode: InputMode::Normal,
            filter_input: Input::default(),
            field_patterns: Vec::new(),
            nodes,
        }
    }

    fn collect_node_paths(node: &CgroupNode, paths: &mut Vec<PathBuf>) {
        paths.push(node.path.clone());
        for child in &node.children {
            Self::collect_node_paths(child, paths);
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

        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Filter => self.handle_filter_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        let rows = self.rows();
        let display_items = self.build_display_items(&rows);
        let num_items = display_items.len();

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('r') => self.rescan(),
            KeyCode::Char('f') => {
                self.input_mode = InputMode::Filter;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(num_items.saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected = num_items.saturating_sub(1);
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Toggle expansion on the parent cgroup (works for both cgroup rows and field lines)
                if let Some(item) = display_items.get(self.selected) {
                    let row_index = item.parent_cgroup_row_index;
                    if let Some(row) = rows.get(row_index)
                        && row.has_children
                    {
                        if !self.expanded.remove(&row.path) {
                            self.expanded.insert(row.path.clone());
                        }
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                // Expand the parent cgroup (works for both cgroup rows and field lines)
                if let Some(item) = display_items.get(self.selected) {
                    let row_index = item.parent_cgroup_row_index;
                    if let Some(row) = rows.get(row_index)
                        && row.has_children
                    {
                        self.expanded.insert(row.path.clone());
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // Collapse or navigate to parent
                if let Some(item) = display_items.get(self.selected) {
                    let row_index = item.parent_cgroup_row_index;
                    if let Some(row) = rows.get(row_index) {
                        if row.expanded {
                            // Collapse the parent cgroup
                            self.expanded.remove(&row.path);
                        } else if let Some(parent_idx) = rows[..row_index]
                            .iter()
                            .rposition(|r| r.depth < row.depth)
                        {
                            // Navigate to the parent row
                            if let Some(parent_display_idx) = display_items
                                .iter()
                                .position(|item| item.cgroup_row_index == Some(parent_idx))
                            {
                                self.selected = parent_display_idx;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('E') => self.expand_all(),
            KeyCode::Char('C') => self.collapse_all(),
            _ => {}
        }
        false
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                let input = self.filter_input.value();
                self.field_patterns = filter::parse_field_patterns(input);
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            _ => {
                // Let tui-input handle all other input
                self.filter_input.handle_event(&Event::Key(key));
            }
        }
        false
    }

    fn rescan(&mut self) {
        if let Ok(data) = cgroup::scan(&self.root) {
            self.data = data;
            self.nodes.clear();
            Self::collect_node_paths(&self.data.root, &mut self.nodes);
            // Clamp selected to valid range after rescan
            let rows = self.rows();
            let display_items = self.build_display_items(&rows);
            self.selected = self.selected.min(display_items.len().saturating_sub(1));
        }
    }

    fn expand_all(&mut self) {
        let mut all_paths = HashSet::new();
        Self::collect_all_paths(&self.data.root, &mut all_paths);
        self.expanded = all_paths;
    }

    fn collapse_all(&mut self) {
        self.expanded.clear();
        self.expanded.insert(self.data.root.path.clone());
    }

    fn collect_all_paths(node: &CgroupNode, paths: &mut HashSet<PathBuf>) {
        if !node.children.is_empty() {
            paths.insert(node.path.clone());
            for child in &node.children {
                Self::collect_all_paths(child, paths);
            }
        }
    }

    fn find_node<'a>(&'a self, path: &Path) -> Option<&'a CgroupNode> {
        Self::find_node_recursive(&self.data.root, path)
    }

    fn find_node_recursive<'a>(node: &'a CgroupNode, path: &Path) -> Option<&'a CgroupNode> {
        if node.path == path {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = Self::find_node_recursive(child, path) {
                return Some(found);
            }
        }
        None
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
        let constraints = match self.input_mode {
            InputMode::Normal => vec![Constraint::Min(3), Constraint::Length(1)],
            InputMode::Filter => vec![Constraint::Min(3), Constraint::Length(1), Constraint::Length(1)],
        };
        let areas = Layout::vertical(constraints).split(frame.area());

        let rows = self.rows();
        self.draw_tree(frame, areas[0], &rows);
        self.draw_footer(frame, areas[1]);

        if let InputMode::Filter = self.input_mode {
            self.draw_filter_input(frame, areas[2]);
        }
    }

    /// Builds the full list of display items (cgroup rows + field lines)
    fn build_display_items(&self, rows: &[Row]) -> Vec<DisplayItem> {
        let mut items: Vec<DisplayItem> = Vec::new();

        for (row_index, row) in rows.iter().enumerate() {
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

            items.push(DisplayItem {
                line: Line::from(spans),
                cgroup_row_index: Some(row_index),
                parent_cgroup_row_index: row_index,
            });

            // If we have field patterns, show the matching fields
            if !self.field_patterns.is_empty() {
                if let Some(node) = self.find_node(&row.path) {
                    let fields = filter::filter_node_fields(node, &self.field_patterns);
                    for field in fields {
                        let field_prefix = if row.depth == 0 {
                            "    ".to_string()
                        } else {
                            let mut prefix = String::new();
                            for &draw_line in &row.ancestor_lines {
                                if draw_line {
                                    prefix.push_str("│   ");
                                } else {
                                    prefix.push_str("    ");
                                }
                            }
                            prefix.push_str("    ");
                            prefix
                        };

                        // Handle multiline values
                        let lines: Vec<&str> = field.value.lines().collect();
                        if lines.is_empty() {
                            items.push(DisplayItem {
                                line: Line::from(vec![
                                    Span::styled(field_prefix.clone(), Style::default().fg(Color::DarkGray)),
                                    Span::styled(field.name.clone(), Style::default().fg(Color::Yellow)),
                                    Span::raw(" = "),
                                ]),
                                cgroup_row_index: None,
                                parent_cgroup_row_index: row_index,
                            });
                        } else if lines.len() == 1 {
                            items.push(DisplayItem {
                                line: Line::from(vec![
                                    Span::styled(field_prefix.clone(), Style::default().fg(Color::DarkGray)),
                                    Span::styled(field.name.clone(), Style::default().fg(Color::Yellow)),
                                    Span::raw(" = "),
                                    Span::styled(field.value.clone(), Style::default().fg(Color::Green)),
                                ]),
                                cgroup_row_index: None,
                                parent_cgroup_row_index: row_index,
                            });
                        } else {
                            // First line with field name
                            items.push(DisplayItem {
                                line: Line::from(vec![
                                    Span::styled(field_prefix.clone(), Style::default().fg(Color::DarkGray)),
                                    Span::styled(field.name.clone(), Style::default().fg(Color::Yellow)),
                                    Span::raw(" ="),
                                ]),
                                cgroup_row_index: None,
                                parent_cgroup_row_index: row_index,
                            });
                            // Subsequent lines indented
                            for line in lines {
                                items.push(DisplayItem {
                                    line: Line::from(vec![
                                        Span::styled(format!("{}    ", &field_prefix), Style::default().fg(Color::DarkGray)),
                                        Span::styled(line.to_string(), Style::default().fg(Color::Green)),
                                    ]),
                                    cgroup_row_index: None,
                                    parent_cgroup_row_index: row_index,
                                });
                            }
                        }
                    }
                }
            }
        }

        items
    }

    fn draw_tree(&mut self, frame: &mut Frame, area: Rect, rows: &[Row]) {
        let display_items = self.build_display_items(rows);

        // Clamp selection to valid range
        self.selected = self.selected.min(display_items.len().saturating_sub(1));

        let list_items: Vec<ListItem> = display_items
            .iter()
            .map(|item| ListItem::new(item.line.clone()))
            .collect();

        let list = List::new(list_items).highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let help = if !self.field_patterns.is_empty() {
            format!(
                " q/esc quit · ↑↓/jk move · ←→/hl collapse/expand · enter/space toggle · E expand all · C collapse all · f filter [{}] · r refresh",
                self.field_patterns.join(",")
            )
        } else {
            " q/esc quit · ↑↓/jk move · ←→/hl collapse/expand · enter/space toggle · E expand all · C collapse all · f filter · r refresh".to_string()
        };
        frame.render_widget(
            Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
            area,
        );
    }

    fn draw_filter_input(&mut self, frame: &mut Frame, area: Rect) {
        let width = area.width.max(3) - 3; // For cursor
        let scroll = self.filter_input.visual_scroll(width as usize);
        let input_text = format!(" Filter: {}", self.filter_input.value());

        let input_widget = Paragraph::new(input_text)
            .style(Style::default().fg(Color::White))
            .scroll((0, scroll as u16));

        frame.render_widget(input_widget, area);

        // Render cursor
        frame.set_cursor_position((
            area.x + (self.filter_input.visual_cursor().max(scroll) - scroll) as u16 + 9, // " Filter: " = 9 chars
            area.y,
        ));
    }
}
