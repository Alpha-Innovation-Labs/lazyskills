use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{collections::HashSet, fs};

use crossterm::event::{
    KeyCode, KeyEvent as CrosstermKeyEvent, KeyEventState, MouseButton,
    MouseEvent as CrosstermMouseEvent, MouseEventKind,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};
use ratkit::prelude::{
    run, CoordinatorAction, CoordinatorApp, CoordinatorEvent, LayoutResult, MouseEvent,
    RunnerConfig,
};
use ratkit::primitives::menu_bar::{MenuBar, MenuItem};
use ratkit::primitives::pane::Pane;
use ratkit::primitives::resizable_grid::{
    PaneId, ResizableGrid, ResizableGridWidget, ResizableGridWidgetState,
};
use ratkit::widgets::markdown_preview::{
    CacheState, CollapseState, DisplaySettings, DoubleClickState, ExpandableState, GitStatsState,
    MarkdownEvent, MarkdownWidget, ScrollState, SelectionState, SourceState, VimState,
};
use ratkit::widgets::{HotkeyFooter, HotkeyItem};

const DEFAULT_SKILL_PATH: &str = ".agents/skills/ratkit/SKILL.md";
const ROOT_AGENTS_PATH: &str = ".agents";
const TERMINAL_ICON: &str = "";
const YAZI_CYAN: Color = Color::Rgb(3, 169, 244);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusPane {
    Tree,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppView {
    Project,
    Search,
}

#[derive(Clone, Debug)]
struct SkillTreeNode {
    dir_name: String,
    display_name: String,
    skill_file: Option<PathBuf>,
    children: Vec<SkillTreeNode>,
}

struct SkillPreviewApp {
    widget: MarkdownWidget<'static>,
    menu: MenuBar,
    skills_nodes: Vec<SkillTreeNode>,
    skills_selected_path: Option<Vec<usize>>,
    skills_expanded: HashSet<Vec<usize>>,
    skills_offset: usize,
    source_path: PathBuf,
    preview_title: String,
    show_toc: bool,
    current_view: AppView,
    focus: FocusPane,
    grid_layout: ResizableGrid,
    grid_state: ResizableGridWidgetState,
    navbar_area: Rect,
    grid_area: Rect,
    tree_pane_id: PaneId,
    preview_pane_id: PaneId,
    tree_area: Rect,
    tree_content_area: Rect,
    markdown_area: Rect,
    markdown_inner_area: Rect,
    last_move_processed: Instant,
    toast_message: Option<String>,
    toast_expires_at: Option<Instant>,
    show_hotkeys_modal: bool,
    pending_preview_path: Option<PathBuf>,
    pending_preview_since: Option<Instant>,
}

impl SkillPreviewApp {
    fn build_menu() -> MenuBar {
        MenuBar::new(vec![
            MenuItem::new("Project [1]", 0),
            MenuItem::new("Search [2]", 1),
        ])
        .with_selected(0)
    }

    fn set_menu_selection(&mut self, target: usize) {
        for (idx, item) in self.menu.items.iter_mut().enumerate() {
            item.selected = idx == target;
        }
    }

    fn set_view(&mut self, view: AppView) {
        self.current_view = view;
        match view {
            AppView::Project => self.set_menu_selection(0),
            AppView::Search => self.set_menu_selection(1),
        }
    }

    fn build_widget(source: SourceState, show_toc: bool) -> MarkdownWidget<'static> {
        let markdown_content = source.content().unwrap_or_default().to_owned();

        let mut scroll = ScrollState::default();
        scroll.update_total_lines(markdown_content.lines().count().max(1));

        let mut display = DisplaySettings::default();
        let _ = display.set_show_document_line_numbers(true);

        MarkdownWidget::new(
            markdown_content,
            scroll,
            source,
            CacheState::default(),
            display,
            CollapseState::default(),
            ExpandableState::default(),
            GitStatsState::default(),
            VimState::default(),
            SelectionState::default(),
            DoubleClickState::default(),
        )
        .with_has_pane(false)
        .show_toc(show_toc)
        .show_scrollbar(true)
        .show_statusline(true)
    }

    fn new(source_path: PathBuf, source: SourceState, skills_nodes: Vec<SkillTreeNode>) -> Self {
        let show_toc = true;
        let widget = Self::build_widget(source, show_toc);
        let preview_title = extract_skill_name_from_frontmatter(&source_path)
            .unwrap_or_else(|| fallback_title_from_path(&source_path));

        let mut skills_expanded = HashSet::new();
        collect_expanded_skill_paths(&skills_nodes, &mut Vec::new(), &mut skills_expanded);

        let mut grid_layout = ResizableGrid::new(0);
        let preview_pane_id = grid_layout.split_pane_vertically(0).unwrap_or(0);
        let _ = grid_layout.resize_divider(0, 20);

        let mut app = Self {
            widget,
            menu: Self::build_menu(),
            skills_nodes,
            skills_selected_path: Some(vec![0]),
            skills_expanded,
            skills_offset: 0,
            source_path,
            preview_title,
            show_toc,
            current_view: AppView::Project,
            focus: FocusPane::Tree,
            grid_layout,
            grid_state: ResizableGridWidgetState::default(),
            navbar_area: Rect::default(),
            grid_area: Rect::default(),
            tree_pane_id: 0,
            preview_pane_id,
            tree_area: Rect::default(),
            tree_content_area: Rect::default(),
            markdown_area: Rect::default(),
            markdown_inner_area: Rect::default(),
            last_move_processed: Instant::now(),
            toast_message: None,
            toast_expires_at: None,
            show_hotkeys_modal: false,
            pending_preview_path: None,
            pending_preview_since: None,
        };

        app.open_selected_file_immediate();
        app
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = Some(message.into());
        self.toast_expires_at = Some(Instant::now() + Duration::from_secs(2));
    }

    fn clear_expired_toast(&mut self) -> bool {
        if let Some(expires_at) = self.toast_expires_at {
            if Instant::now() >= expires_at {
                self.toast_message = None;
                self.toast_expires_at = None;
                return true;
            }
        }
        false
    }

    fn handle_grid_mouse(&mut self, mouse: MouseEvent) {
        let crossterm_mouse = CrosstermMouseEvent {
            kind: mouse.kind,
            column: mouse.column,
            row: mouse.row,
            modifiers: mouse.modifiers,
        };

        let mut widget =
            ResizableGridWidget::new(self.grid_layout.clone()).with_state(self.grid_state);
        widget.handle_mouse(crossterm_mouse, self.grid_area);
        self.grid_state = widget.state();
        self.grid_layout = widget.layout().clone();
    }

    fn update_pane_areas_from_grid(&mut self) {
        let panes = self.grid_layout.get_panes(self.grid_area);
        if let Some(pane) = panes.iter().find(|pane| pane.id == self.tree_pane_id) {
            self.tree_area = pane.area;
        }
        if let Some(pane) = panes.iter().find(|pane| pane.id == self.preview_pane_id) {
            self.markdown_area = pane.area;
        }
    }

    fn queue_open_selected_file(&mut self) {
        let selected_path = {
            let Some(node) = self.selected_skill_node() else {
                return;
            };
            let Some(path) = &node.skill_file else {
                return;
            };
            path.clone()
        };

        if selected_path == self.source_path {
            return;
        }
        self.pending_preview_path = Some(selected_path);
        self.pending_preview_since = Some(Instant::now());
    }

    fn open_selected_file_immediate(&mut self) {
        let Some(node) = self.selected_skill_node() else {
            return;
        };
        let Some(path) = &node.skill_file else {
            return;
        };
        if let Ok(source) = load_source_from_path(path) {
            self.source_path = path.clone();
            self.preview_title = extract_skill_name_from_frontmatter(&self.source_path)
                .unwrap_or_else(|| fallback_title_from_path(&self.source_path));
            self.widget = Self::build_widget(source, self.show_toc);
        }
    }

    fn selected_skill_node(&self) -> Option<&SkillTreeNode> {
        let path = self.skills_selected_path.as_ref()?;
        let mut nodes = &self.skills_nodes;
        let mut current = None;
        for idx in path {
            let node = nodes.get(*idx)?;
            current = Some(node);
            nodes = &node.children;
        }
        current
    }

    fn collect_visible_skill_paths(
        nodes: &[SkillTreeNode],
        expanded: &HashSet<Vec<usize>>,
        base: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        for (idx, node) in nodes.iter().enumerate() {
            base.push(idx);
            let path = base.clone();
            out.push(path.clone());
            if expanded.contains(&path) {
                Self::collect_visible_skill_paths(&node.children, expanded, base, out);
            }
            let _ = base.pop();
        }
    }

    fn visible_skill_paths(&self) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        Self::collect_visible_skill_paths(
            &self.skills_nodes,
            &self.skills_expanded,
            &mut Vec::new(),
            &mut out,
        );
        out
    }

    fn ensure_skill_selection_visible(&mut self) {
        let visible = self.visible_skill_paths();
        if visible.is_empty() {
            self.skills_selected_path = None;
            self.skills_offset = 0;
            return;
        }
        let valid = self
            .skills_selected_path
            .as_ref()
            .map(|p| visible.iter().any(|v| v == p))
            .unwrap_or(false);
        if !valid {
            self.skills_selected_path = Some(visible[0].clone());
        }
    }

    fn select_next_skill(&mut self) {
        let visible = self.visible_skill_paths();
        if visible.is_empty() {
            return;
        }
        let current = self
            .skills_selected_path
            .as_ref()
            .and_then(|p| visible.iter().position(|v| v == p))
            .unwrap_or(0);
        let next = (current + 1).min(visible.len().saturating_sub(1));
        self.skills_selected_path = Some(visible[next].clone());
    }

    fn select_prev_skill(&mut self) {
        let visible = self.visible_skill_paths();
        if visible.is_empty() {
            return;
        }
        let current = self
            .skills_selected_path
            .as_ref()
            .and_then(|p| visible.iter().position(|v| v == p))
            .unwrap_or(0);
        let prev = current.saturating_sub(1);
        self.skills_selected_path = Some(visible[prev].clone());
    }

    fn expand_selected_skill(&mut self) {
        if let Some(path) = self.skills_selected_path.clone() {
            self.skills_expanded.insert(path);
        }
    }

    fn collapse_selected_skill(&mut self) {
        if let Some(path) = self.skills_selected_path.clone() {
            if self.skills_expanded.contains(&path) {
                self.skills_expanded.remove(&path);
            } else if path.len() > 1 {
                let mut parent = path;
                parent.pop();
                self.skills_selected_path = Some(parent);
            }
        }
    }

    fn flush_pending_preview_if_ready(&mut self) -> bool {
        const PREVIEW_DEBOUNCE_MS: u64 = 200;

        let Some(pending_since) = self.pending_preview_since else {
            return false;
        };
        if pending_since.elapsed() < Duration::from_millis(PREVIEW_DEBOUNCE_MS) {
            return false;
        }

        let Some(selected_path) = self.pending_preview_path.take() else {
            self.pending_preview_since = None;
            return false;
        };
        self.pending_preview_since = None;

        if let Ok(source) = load_source_from_path(&selected_path) {
            self.source_path = selected_path;
            self.preview_title = extract_skill_name_from_frontmatter(&self.source_path)
                .unwrap_or_else(|| fallback_title_from_path(&self.source_path));
            self.widget = Self::build_widget(source, self.show_toc);
            return true;
        }
        false
    }

    fn is_in_rect(rect: Rect, x: u16, y: u16) -> bool {
        x >= rect.x
            && y >= rect.y
            && x < rect.x.saturating_add(rect.width)
            && y < rect.y.saturating_add(rect.height)
    }
}

impl CoordinatorApp for SkillPreviewApp {
    fn on_event(&mut self, event: CoordinatorEvent) -> LayoutResult<CoordinatorAction> {
        match event {
            CoordinatorEvent::Keyboard(key) => {
                if !key.is_key_down() {
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Char('q')
                    || (key.key_code == KeyCode::Char('c')
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL))
                {
                    return Ok(CoordinatorAction::Quit);
                }

                if key.key_code == KeyCode::Esc && self.show_hotkeys_modal {
                    self.show_hotkeys_modal = false;
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('?') {
                    self.show_hotkeys_modal = !self.show_hotkeys_modal;
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.show_hotkeys_modal {
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Char('1') {
                    self.set_view(AppView::Project);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('2') {
                    self.set_view(AppView::Search);
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.current_view == AppView::Search {
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Char(']') {
                    self.show_toc = !self.show_toc;
                    if let Ok(source) = load_source_from_path(&self.source_path) {
                        self.widget = Self::build_widget(source, self.show_toc);
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Tab {
                    self.focus = match self.focus {
                        FocusPane::Tree => FocusPane::Preview,
                        FocusPane::Preview => FocusPane::Tree,
                    };
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.focus == FocusPane::Tree {
                    if self.pending_preview_path.is_some() {
                        self.pending_preview_since = Some(Instant::now());
                    }

                    self.ensure_skill_selection_visible();
                    let selected_before = self.skills_selected_path.clone();
                    match key.key_code {
                        KeyCode::Down | KeyCode::Char('j') => self.select_next_skill(),
                        KeyCode::Up | KeyCode::Char('k') => self.select_prev_skill(),
                        KeyCode::Right | KeyCode::Char('l') => self.expand_selected_skill(),
                        KeyCode::Left | KeyCode::Char('h') => self.collapse_selected_skill(),
                        _ => {}
                    }
                    let selected_after = self.skills_selected_path.clone();
                    if selected_after != selected_before || key.key_code == KeyCode::Enter {
                        self.queue_open_selected_file();
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                let key_event = CrosstermKeyEvent {
                    code: key.key_code,
                    modifiers: key.modifiers,
                    kind: key.kind,
                    state: KeyEventState::NONE,
                };

                let markdown_event = self.widget.handle_key(key_event);
                let copied_chars = match &markdown_event {
                    MarkdownEvent::Copied { text } => Some(text.chars().count()),
                    _ => None,
                };
                if let Some(copied_chars) = copied_chars {
                    self.show_toast(format!("Copied {} chars to clipboard", copied_chars));
                }
                if matches!(markdown_event, MarkdownEvent::None) {
                    Ok(CoordinatorAction::Continue)
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Mouse(mouse) => {
                if self.show_hotkeys_modal {
                    return Ok(CoordinatorAction::Continue);
                }

                if Self::is_in_rect(self.navbar_area, mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::Moved => self.menu.update_hover(mouse.column, mouse.row),
                        MouseEventKind::Down(MouseButton::Left) => {
                            if let Some(index) = self.menu.handle_click(mouse.column, mouse.row) {
                                match index {
                                    0 => self.set_view(AppView::Project),
                                    1 => self.set_view(AppView::Search),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.current_view == AppView::Search {
                    return Ok(CoordinatorAction::Continue);
                }

                let was_grid_dragging = self.grid_layout.is_dragging();
                self.handle_grid_mouse(mouse);
                self.update_pane_areas_from_grid();

                if was_grid_dragging || self.grid_layout.is_dragging() {
                    return Ok(CoordinatorAction::Redraw);
                }

                if !Self::is_in_rect(self.markdown_inner_area, mouse.column, mouse.row) {
                    return Ok(CoordinatorAction::Continue);
                }

                let is_moved = matches!(mouse.kind, crossterm::event::MouseEventKind::Moved);
                if is_moved {
                    if self.last_move_processed.elapsed() < Duration::from_millis(24) {
                        return Ok(CoordinatorAction::Continue);
                    }
                    self.last_move_processed = Instant::now();
                }

                let mouse_event = CrosstermMouseEvent {
                    kind: mouse.kind,
                    column: mouse.column,
                    row: mouse.row,
                    modifiers: mouse.modifiers,
                };

                let markdown_event = self
                    .widget
                    .handle_mouse(mouse_event, self.markdown_inner_area);
                let copied_chars = match &markdown_event {
                    MarkdownEvent::Copied { text } => Some(text.chars().count()),
                    _ => None,
                };
                if let Some(copied_chars) = copied_chars {
                    self.show_toast(format!("Copied {} chars to clipboard", copied_chars));
                }

                if is_moved {
                    if matches!(markdown_event, MarkdownEvent::TocHoverChanged { .. }) {
                        Ok(CoordinatorAction::Redraw)
                    } else {
                        Ok(CoordinatorAction::Continue)
                    }
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Tick(_) => {
                let toast_changed = self.clear_expired_toast();
                let preview_changed = self.flush_pending_preview_if_ready();
                if toast_changed || preview_changed {
                    Ok(CoordinatorAction::Redraw)
                } else {
                    Ok(CoordinatorAction::Continue)
                }
            }
            CoordinatorEvent::Resize(_) => Ok(CoordinatorAction::Redraw),
            _ => Ok(CoordinatorAction::Continue),
        }
    }

    fn on_draw(&mut self, frame: &mut Frame) {
        let root_area = frame.area();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(root_area);
        self.navbar_area = rows[0];
        self.grid_area = rows[1];
        let footer_area = rows[2];

        self.menu.render(frame, self.navbar_area);

        let footer = HotkeyFooter::new(vec![
            HotkeyItem::new("q", "quit"),
            HotkeyItem::new(
                "]",
                if self.show_toc {
                    "hide toc"
                } else {
                    "show toc"
                },
            ),
            HotkeyItem::new("?", "hotkeys"),
        ]);
        footer.render(frame, footer_area);

        if self.current_view == AppView::Search {
            frame.render_widget(
                Paragraph::new("Search view").style(Style::default().fg(Color::White)),
                self.grid_area,
            );
        } else {
            let grid_widget =
                ResizableGridWidget::new(self.grid_layout.clone()).with_state(self.grid_state);
            self.grid_state = grid_widget.state();
            self.grid_layout = grid_widget.layout().clone();
            frame.render_widget(grid_widget, self.grid_area);

            self.update_pane_areas_from_grid();
            self.markdown_inner_area = Rect::default();

            if self.tree_area.width > 0 && self.tree_area.height > 0 {
                let tree_border = if self.focus == FocusPane::Tree {
                    Color::Blue
                } else {
                    Color::White
                };
                let tree_pane = Pane::new("Skills")
                    .with_icon(TERMINAL_ICON)
                    .border_style(Style::default().fg(tree_border));
                let (tree_inner, _) = tree_pane.render_block(frame, self.tree_area);
                self.tree_content_area = tree_inner;

                self.ensure_skill_selection_visible();
                let visible = self.visible_skill_paths();
                let max_rows = self.tree_content_area.height as usize;
                if let Some(selected) = &self.skills_selected_path {
                    if let Some(idx) = visible.iter().position(|p| p == selected) {
                        if idx < self.skills_offset {
                            self.skills_offset = idx;
                        } else if idx >= self.skills_offset.saturating_add(max_rows) {
                            self.skills_offset = idx.saturating_sub(max_rows.saturating_sub(1));
                        }
                    }
                }

                let mut lines = Vec::new();
                let line_width = self.tree_content_area.width as usize;
                for path in visible.iter().skip(self.skills_offset).take(max_rows) {
                    if let Some(node) = skill_node_at_path(&self.skills_nodes, path) {
                        let depth = path.len().saturating_sub(1);
                        let indent = "  ".repeat(depth);
                        let is_selected = self
                            .skills_selected_path
                            .as_ref()
                            .map(|p| p == path)
                            .unwrap_or(false);
                        let is_expanded = self.skills_expanded.contains(path);
                        let disclosure = if node.children.is_empty() {
                            " "
                        } else if is_expanded {
                            "▾"
                        } else {
                            "▸"
                        };
                        let row_style = if is_selected {
                            Style::default().fg(Color::Black).bg(YAZI_CYAN)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        let text = format!(
                            "{}{} {} {}",
                            indent, disclosure, TERMINAL_ICON, node.display_name
                        );
                        let padded = if line_width > 0 {
                            format!("{:<width$}", text, width = line_width)
                        } else {
                            text
                        };
                        lines.push(Line::from(Span::styled(padded, row_style)));
                    }
                }

                frame.render_widget(Paragraph::new(lines), self.tree_content_area);
            }

            if self.markdown_area.width > 0 && self.markdown_area.height > 0 {
                let preview_border = if self.focus == FocusPane::Preview {
                    Color::Blue
                } else {
                    Color::White
                };
                let preview_title = if self.focus == FocusPane::Preview {
                    preview_relative_to_skills(&self.source_path)
                } else {
                    self.preview_title.clone()
                };
                let preview_pane = Pane::new(preview_title)
                    .with_icon(TERMINAL_ICON)
                    .border_style(Style::default().fg(preview_border));
                let (preview_inner, _) = preview_pane.render_block(frame, self.markdown_area);
                self.markdown_inner_area = preview_inner;
                if preview_inner.width > 0 && preview_inner.height > 0 {
                    frame.render_widget(&mut self.widget, preview_inner);
                }
            }

            if let Some(message) = &self.toast_message {
                if self.markdown_inner_area.height > 0 {
                    let toast_width =
                        (message.chars().count() as u16 + 2).min(self.markdown_inner_area.width);
                    let toast_area = Rect {
                        x: self.markdown_inner_area.x
                            + self.markdown_inner_area.width.saturating_sub(toast_width) / 2,
                        y: self.markdown_inner_area.y
                            + self.markdown_inner_area.height.saturating_sub(1),
                        width: toast_width,
                        height: 1,
                    };
                    frame.render_widget(
                        Paragraph::new(Line::from(format!(" {}", message)))
                            .style(Style::default().fg(Color::Black).bg(Color::LightGreen)),
                        toast_area,
                    );
                }
            }
        }

        if self.show_hotkeys_modal {
            let modal_width = root_area.width.min(68);
            let modal_height = root_area.height.min(18);
            let modal_area = Rect {
                x: root_area.x + (root_area.width.saturating_sub(modal_width)) / 2,
                y: root_area.y + (root_area.height.saturating_sub(modal_height)) / 2,
                width: modal_width,
                height: modal_height,
            };

            frame.render_widget(Clear, modal_area);

            let hotkeys_pane = Pane::new("Hotkeys")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(Color::Green));
            let hotkeys = vec![
                Line::from("Global"),
                Line::from(" q           quit"),
                Line::from(" ]           toggle toc"),
                Line::from(" ? / esc     open/close hotkeys"),
                Line::from(" 1 / 2       switch Project/Search"),
                Line::from(" tab         switch tree/preview focus"),
                Line::from(""),
                Line::from("File Tree"),
                Line::from(" up/down     move selection"),
                Line::from(" enter       expand/collapse or open file"),
                Line::from(" /           filter tree"),
                Line::from(""),
                Line::from("Markdown"),
                Line::from(" mouse wheel scroll"),
                Line::from(" drag/select copy (shows toast)"),
            ];
            hotkeys_pane.render_paragraph(frame, modal_area, hotkeys);
        }
    }
}

fn load_source_from_path(path: impl AsRef<Path>) -> io::Result<SourceState> {
    let path = path.as_ref();

    let mut source = SourceState::default();
    source.set_source_file(path)?;
    Ok(source)
}

fn fallback_title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Preview")
        .to_string()
}

fn preview_relative_to_skills(path: &Path) -> String {
    let skills_root = PathBuf::from(ROOT_AGENTS_PATH).join("skills");
    if let Ok(relative) = path.strip_prefix(&skills_root) {
        return relative.display().to_string();
    }
    if let Ok(relative) = path.strip_prefix(PathBuf::from(ROOT_AGENTS_PATH)) {
        return relative.display().to_string();
    }
    path.display().to_string()
}

fn extract_skill_name_from_frontmatter(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let value = rest.trim().trim_matches('"').trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn skill_node_at_path<'a>(nodes: &'a [SkillTreeNode], path: &[usize]) -> Option<&'a SkillTreeNode> {
    let mut current_nodes = nodes;
    let mut current_node: Option<&SkillTreeNode> = None;
    for idx in path {
        let node = current_nodes.get(*idx)?;
        current_node = Some(node);
        current_nodes = &node.children;
    }
    current_node
}

fn collect_expanded_skill_paths(
    nodes: &[SkillTreeNode],
    base: &mut Vec<usize>,
    expanded: &mut HashSet<Vec<usize>>,
) {
    for (idx, node) in nodes.iter().enumerate() {
        base.push(idx);
        expanded.insert(base.clone());
        collect_expanded_skill_paths(&node.children, base, expanded);
        let _ = base.pop();
    }
}

fn insert_skill_node(
    nodes: &mut Vec<SkillTreeNode>,
    comps: &[String],
    skill_file: PathBuf,
    skill_name: String,
) {
    if comps.is_empty() {
        return;
    }
    let current = &comps[0];

    let index = if let Some(idx) = nodes.iter().position(|n| &n.dir_name == current) {
        idx
    } else {
        nodes.push(SkillTreeNode {
            dir_name: current.clone(),
            display_name: current.clone(),
            skill_file: None,
            children: Vec::new(),
        });
        nodes.len() - 1
    };

    if comps.len() == 1 {
        nodes[index].skill_file = Some(skill_file);
        nodes[index].display_name = skill_name;
    } else {
        insert_skill_node(
            &mut nodes[index].children,
            &comps[1..],
            skill_file,
            skill_name,
        );
    }
}

fn load_skill_hierarchy() -> io::Result<Vec<SkillTreeNode>> {
    let root = PathBuf::from(ROOT_AGENTS_PATH);
    let skills_root = root.join("skills");
    let start = if skills_root.exists() {
        skills_root
    } else {
        root
    };

    let mut skill_files = Vec::new();
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out)?;
            } else if path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("SKILL.md"))
                .unwrap_or(false)
            {
                out.push(path);
            }
        }
        Ok(())
    }
    collect(&start, &mut skill_files)?;

    let mut nodes = Vec::new();
    for file in skill_files {
        let Some(parent) = file.parent() else {
            continue;
        };
        let Ok(relative) = parent.strip_prefix(&start) else {
            continue;
        };
        let comps: Vec<String> = relative
            .iter()
            .filter_map(|c| c.to_str().map(|s| s.to_string()))
            .collect();
        if comps.is_empty() {
            continue;
        }

        let skill_name = extract_skill_name_from_frontmatter(&file)
            .unwrap_or_else(|| comps.last().cloned().unwrap_or_else(|| "skill".to_string()));
        insert_skill_node(&mut nodes, &comps, file.clone(), skill_name);
    }

    Ok(nodes)
}

fn main() -> io::Result<()> {
    let source_path = PathBuf::from(DEFAULT_SKILL_PATH);
    let source = load_source_from_path(&source_path)?;
    let skills_nodes = load_skill_hierarchy()?;
    let app = SkillPreviewApp::new(source_path, source, skills_nodes);
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(250),
        ..RunnerConfig::default()
    };
    run(app, config)
}
