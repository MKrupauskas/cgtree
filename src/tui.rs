use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::cgroup;
use crate::data::{CgroupData, CgroupNode, FieldEntry};
use crate::filter;

pub fn run(root: &Path, data: CgroupData) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        bail!("interactive view needs a terminal; use `cgtree list` when piping");
    }
    let mut app = App::new(root.to_path_buf(), data);
    // `try_init` rather than `init`: the latter panics if the terminal cannot
    // be set up (e.g. its size is unavailable), which would report a failure
    // this binary can describe properly as a backtrace instead.
    let mut terminal = ratatui::try_init().context("failed to initialize terminal")?;
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

/// One visible row of the tree, in display order.
struct Row<'a> {
    path: &'a Path,
    label: String,
    /// The cgroup's interface files, borrowed from the scanned tree so that
    /// rendering properties does not have to search for the node again.
    fields: &'a [FieldEntry],
    depth: usize,
    has_children: bool,
    expanded: bool,
    /// Which child index this is among its siblings (for drawing tree lines)
    is_last: bool,
    /// For each depth level, whether we need to draw a vertical line
    ancestor_lines: Vec<bool>,
}

/// One line of the list widget: either a cgroup row or one of its properties.
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
    Help,
}

#[derive(Clone, Copy, PartialEq)]
enum PropsMode {
    Hide,
    ShowAll,
    Filtered,
}

pub struct App {
    root: PathBuf,
    data: CgroupData,
    expanded: HashSet<PathBuf>,
    /// Index of the currently selected display item (includes both cgroup rows and field lines)
    selected: usize,
    input_mode: InputMode,
    filter_input: Input,
    field_patterns: Vec<String>,
    /// The user's saved filter patterns (set via 'f' filter input)
    saved_filter_patterns: Vec<String>,
    /// Current props display mode
    props_mode: PropsMode,
    /// Transient message shown in place of the help footer (e.g. a failed rescan).
    status: Option<String>,
}

impl App {
    /// Creates an explorer over an already-scanned hierarchy.
    ///
    /// The binary reaches this through [`run`]. It is public, along with
    /// [`App::handle_key`] and [`App::draw`], so the integration tests can
    /// drive the explorer headlessly against a `TestBackend` — those three
    /// are the same entry points [`App::run`] uses, not test-only scaffolding.
    #[must_use]
    pub fn new(root: PathBuf, data: CgroupData) -> Self {
        let mut expanded = HashSet::new();
        expanded.insert(data.root.path.clone());

        App {
            root,
            data,
            expanded,
            selected: 0,
            input_mode: InputMode::Normal,
            filter_input: Input::default(),
            field_patterns: Vec::new(),
            saved_filter_patterns: Vec::new(),
            props_mode: PropsMode::Hide,
            status: None,
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
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }

        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Filter => self.handle_filter_key(key),
            InputMode::Help => self.handle_help_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        // Any keystroke dismisses a status message; `rescan` re-sets its own.
        self.status = None;

        let rows = self.rows();
        let display_items = self.build_display_items(&rows);
        let num_items = display_items.len();

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('?') => {
                self.input_mode = InputMode::Help;
            }
            KeyCode::Char('r') => self.rescan(),
            KeyCode::Char('f') => {
                self.input_mode = InputMode::Filter;
            }
            KeyCode::Char('p') => {
                self.toggle_props_mode();
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
                if let Some((path, has_children, expanded)) =
                    self.selected_row(&rows, &display_items)
                    && has_children
                {
                    if expanded {
                        self.expanded.remove(&path);
                    } else {
                        self.expanded.insert(path);
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some((path, has_children, _)) = self.selected_row(&rows, &display_items)
                    && has_children
                {
                    self.expanded.insert(path);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // Collapse when expanded; on a collapsed row jump to the parent.
                if let Some(item) = display_items.get(self.selected) {
                    let row_index = item.parent_cgroup_row_index;
                    if let Some(row) = rows.get(row_index) {
                        if row.expanded {
                            let path = row.path.to_path_buf();
                            self.expanded.remove(&path);
                        } else if let Some(parent_idx) =
                            rows[..row_index].iter().rposition(|r| r.depth < row.depth)
                            && let Some(parent_display_idx) = display_items
                                .iter()
                                .position(|item| item.cgroup_row_index == Some(parent_idx))
                        {
                            self.selected = parent_display_idx;
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

    /// The cgroup row the cursor sits on — for a property line, the cgroup it
    /// belongs to. Returns owned data so callers can mutate `expanded` after.
    fn selected_row(
        &self,
        rows: &[Row<'_>],
        items: &[DisplayItem],
    ) -> Option<(PathBuf, bool, bool)> {
        let item = items.get(self.selected)?;
        let row = rows.get(item.parent_cgroup_row_index)?;
        Some((row.path.to_path_buf(), row.has_children, row.expanded))
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                let input = self.filter_input.value();
                self.saved_filter_patterns = filter::parse_field_patterns(input);

                // Switch to filtered mode if we have patterns, otherwise hide
                if self.saved_filter_patterns.is_empty() {
                    self.props_mode = PropsMode::Hide;
                    self.field_patterns = Vec::new();
                } else {
                    self.props_mode = PropsMode::Filtered;
                    self.field_patterns = self.saved_filter_patterns.clone();
                }

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

    fn handle_help_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q' | '?') | KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            _ => {}
        }
        false
    }

    fn toggle_props_mode(&mut self) {
        self.props_mode = match self.props_mode {
            PropsMode::Hide => PropsMode::ShowAll,
            PropsMode::ShowAll => {
                // If we have saved filter patterns, go to Filtered mode
                // Otherwise, cycle back to Hide
                if self.saved_filter_patterns.is_empty() {
                    PropsMode::Hide
                } else {
                    PropsMode::Filtered
                }
            }
            PropsMode::Filtered => PropsMode::Hide,
        };

        // Update field_patterns based on the new mode
        self.field_patterns = match self.props_mode {
            PropsMode::Hide => Vec::new(),
            PropsMode::ShowAll => vec![String::from("*")],
            PropsMode::Filtered => self.saved_filter_patterns.clone(),
        };
    }

    /// Re-reads the hierarchy from disk, keeping the current expansion state.
    ///
    /// A failed rescan leaves the previous snapshot on screen and reports the
    /// reason in the footer — silently doing nothing would look like `r` was
    /// not registered at all.
    fn rescan(&mut self) {
        match cgroup::scan(&self.root) {
            Ok(data) => {
                self.data = data;
                self.status = None;
                // Clamp selection: the refreshed tree may be smaller.
                let rows = self.rows();
                let display_items = self.build_display_items(&rows);
                self.selected = self.selected.min(display_items.len().saturating_sub(1));
            }
            Err(err) => self.status = Some(format!("rescan failed: {err}")),
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

    /// Flattens the expanded portion of the tree into display order.
    fn rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        self.flatten(&self.data.root, 0, &mut rows, &[], true);
        rows
    }

    fn flatten<'a>(
        &'a self,
        node: &'a CgroupNode,
        depth: usize,
        rows: &mut Vec<Row<'a>>,
        ancestor_lines: &[bool],
        is_last: bool,
    ) {
        let expanded = self.expanded.contains(&node.path);
        let has_children = !node.children.is_empty();

        rows.push(Row {
            path: &node.path,
            fields: &node.fields,
            label: if has_children {
                format!("{}/", node.name)
            } else {
                node.name.clone()
            },
            depth,
            has_children,
            expanded,
            is_last,
            ancestor_lines: ancestor_lines.to_vec(),
        });

        if expanded && has_children {
            let new_ancestor_lines = if depth > 0 {
                let mut lines = ancestor_lines.to_vec();
                lines.push(!is_last);
                lines
            } else {
                // Root's children (depth 1) should have no indentation
                Vec::new()
            };

            for (i, child) in node.children.iter().enumerate() {
                let is_last_child = i == node.children.len() - 1;
                self.flatten(child, depth + 1, rows, &new_ancestor_lines, is_last_child);
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        if let InputMode::Help = self.input_mode {
            self.draw_help(frame, frame.area());
            return;
        }

        let constraints = match self.input_mode {
            InputMode::Normal => vec![Constraint::Min(3), Constraint::Length(1)],
            InputMode::Filter => vec![
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(1),
            ],
            InputMode::Help => unreachable!(),
        };
        let areas = Layout::vertical(constraints).split(frame.area());

        // Clamp before rendering: the selection may point past the end after a
        // collapse or a rescan shrank the tree.
        let items = {
            let rows = self.rows();
            self.build_display_items(&rows)
        };
        self.selected = self.selected.min(items.len().saturating_sub(1));

        self.draw_tree(frame, areas[0], items);
        self.draw_footer(frame, areas[1]);

        if let InputMode::Filter = self.input_mode {
            self.draw_filter_input(frame, areas[2]);
        }
    }

    /// Builds the full list of display items (cgroup rows + field lines)
    fn build_display_items(&self, rows: &[Row]) -> Vec<DisplayItem> {
        let mut items: Vec<DisplayItem> = Vec::new();

        for (row_index, row) in rows.iter().enumerate() {
            items.push(DisplayItem {
                line: Line::from(tree_spans(row)),
                cgroup_row_index: Some(row_index),
                parent_cgroup_row_index: row_index,
            });

            // Properties belonging to this cgroup, indented under it.
            let prefix = field_prefix(row);
            for field in filter::matching_fields(row.fields, &self.field_patterns) {
                for line in field_lines(&prefix, field) {
                    items.push(DisplayItem {
                        line,
                        cgroup_row_index: None,
                        parent_cgroup_row_index: row_index,
                    });
                }
            }
        }

        items
    }

    fn draw_tree(&self, frame: &mut Frame, area: Rect, display_items: Vec<DisplayItem>) {
        let list_items: Vec<ListItem> = display_items
            .into_iter()
            .map(|item| ListItem::new(item.line))
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
        if let Some(status) = &self.status {
            frame.render_widget(
                Line::from(Span::styled(
                    format!(" {status}"),
                    Style::default().fg(Color::Red),
                )),
                area,
            );
            return;
        }

        let props_status = match self.props_mode {
            PropsMode::Hide => "p props:hide",
            PropsMode::ShowAll => "p props:show-all",
            PropsMode::Filtered => {
                if self.saved_filter_patterns.is_empty() {
                    "p props:hide"
                } else {
                    "p props:filtered"
                }
            }
        };

        let filter_info = if self.saved_filter_patterns.is_empty() {
            "f filter".to_string()
        } else {
            format!("f filter:{}", self.saved_filter_patterns.join(","))
        };

        let help = format!(
            " ? help · q/esc quit · ↑↓/jk move · ←→/hl collapse/expand · enter/space toggle · E expand all · C collapse all · {props_status} · {filter_info} · r refresh"
        );

        frame.render_widget(
            Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
            area,
        );
    }

    fn draw_filter_input(&self, frame: &mut Frame, area: Rect) {
        const PROMPT: &str = " Property filter: ";

        // Reserve the prompt plus a column for the cursor itself.
        let text_width = usize::from(area.width).saturating_sub(PROMPT.len() + 1);
        let scroll = self.filter_input.visual_scroll(text_width);

        frame.render_widget(
            Paragraph::new(format!("{PROMPT}{}", self.filter_input.value()))
                .style(Style::default().fg(Color::White))
                .scroll((0, u16::try_from(scroll).unwrap_or(u16::MAX))),
            area,
        );

        let cursor_col = PROMPT.len() + self.filter_input.visual_cursor().saturating_sub(scroll);
        frame.set_cursor_position((
            area.x
                + u16::try_from(cursor_col)
                    .unwrap_or(u16::MAX)
                    .min(area.width - 1),
            area.y,
        ));
    }

    fn draw_help(&self, frame: &mut Frame, area: Rect) {
        let props_status = match self.props_mode {
            PropsMode::Hide => "hide",
            PropsMode::ShowAll => "show-all",
            PropsMode::Filtered => "filtered",
        };

        let filter_status = if self.saved_filter_patterns.is_empty() {
            "none".to_string()
        } else {
            self.saved_filter_patterns.join(",")
        };

        let help_text = vec![
            Line::from(vec![
                Span::styled(
                    "cgtree ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("- Interactive cgroup hierarchy explorer"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Navigation",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  ↑/↓  or  j/k        Move selection up/down"),
            Line::from("  →/l                 Expand current node"),
            Line::from("  ←/h                 Collapse current node (or jump to parent)"),
            Line::from("  Enter  or  Space    Toggle expansion"),
            Line::from("  E                   Expand all nodes"),
            Line::from("  C                   Collapse all nodes"),
            Line::from("  g  /  G             Jump to top / bottom"),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Properties Display ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("(mode:{props_status})"),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from("  p                   Toggle props display mode:"),
            Line::from("                        • Hide - No properties shown (default)"),
            Line::from("                        • Show all - Display all cgroup properties"),
            Line::from("                        • Filtered - Show properties matching your filter"),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Property Filtering ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("(filter:{filter_status})"),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from("  f                   Open property filter input"),
            Line::from("                      Type comma-separated patterns (e.g., memory,cpu)"),
            Line::from("                      Use * to show all properties"),
            Line::from("                      Patterns use substring matching"),
            Line::from("  Enter               Apply filter and switch to filtered mode"),
            Line::from("  Esc                 Cancel filter input"),
            Line::from(""),
            Line::from(Span::styled(
                "Other",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  r                   Rescan the cgroup hierarchy"),
            Line::from("  ?                   Show this help screen"),
            Line::from("  q  or  Esc          Quit (or close help)"),
            Line::from(""),
            Line::from(Span::styled(
                "Press ? or Esc to close this help",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let help = Paragraph::new(help_text);
        frame.render_widget(help, area);
    }
}

const GUIDE: Style = Style::new().fg(Color::DarkGray);

/// The `tree`-style guides and label for one cgroup row.
fn tree_spans(row: &Row) -> Vec<Span<'static>> {
    if row.depth == 0 {
        return vec![Span::raw(row.label.clone())];
    }

    let mut spans: Vec<Span<'static>> = row
        .ancestor_lines
        .iter()
        .map(|&draw_line| {
            if draw_line {
                Span::styled("│   ", GUIDE)
            } else {
                Span::raw("    ")
            }
        })
        .collect();
    spans.push(Span::styled(
        if row.is_last {
            "└── "
        } else {
            "├── "
        },
        GUIDE,
    ));
    spans.push(Span::raw(row.label.clone()));
    spans
}

/// The indent that a row's property lines sit at.
fn field_prefix(row: &Row) -> String {
    let mut prefix = String::new();
    if row.depth > 0 {
        for &draw_line in &row.ancestor_lines {
            prefix.push_str(if draw_line { "│   " } else { "    " });
        }
    }
    prefix.push_str("    ");
    prefix
}

/// Renders one property as one or more display lines.
///
/// Single-line values sit on the `name = value` line; multi-line values (such
/// as `memory.stat`) put the name alone and indent the body beneath it.
fn field_lines(prefix: &str, field: &FieldEntry) -> Vec<Line<'static>> {
    let name = || Span::styled(field.name.clone(), Style::default().fg(Color::Yellow));
    let indent = || Span::styled(prefix.to_string(), GUIDE);

    let mut value_lines = field.value.lines();
    let first = value_lines.next();
    let is_multiline = field.value.lines().nth(1).is_some();

    if !is_multiline {
        // Both an empty value and a single-line one render on one line; an
        // empty one simply has nothing after the `=`.
        let mut spans = vec![indent(), name(), Span::raw(" = ")];
        if let Some(value) = first {
            spans.push(Span::styled(
                value.to_string(),
                Style::default().fg(Color::Green),
            ));
        }
        return vec![Line::from(spans)];
    }

    let mut lines = vec![Line::from(vec![indent(), name(), Span::raw(" =")])];
    lines.extend(field.value.lines().map(|line| {
        Line::from(vec![
            Span::styled(format!("{prefix}    "), GUIDE),
            Span::styled(line.to_string(), Style::default().fg(Color::Green)),
        ])
    }));
    lines
}
