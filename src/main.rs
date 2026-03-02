use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{
    KeyCode, KeyEvent as CrosstermKeyEvent, KeyEventState, MouseEvent as CrosstermMouseEvent,
};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
    Frame,
};
use ratkit::prelude::{
    run, CoordinatorAction, CoordinatorApp, CoordinatorEvent, LayoutResult, MouseEvent,
    RunnerConfig,
};
use ratkit::primitives::resizable_grid::{
    PaneId, ResizableGrid, ResizableGridWidget, ResizableGridWidgetState,
};
use ratkit::widgets::markdown_preview::{
    CacheState, CollapseState, DisplaySettings, DoubleClickState, ExpandableState, GitStatsState,
    MarkdownEvent, MarkdownWidget, ScrollState, SelectionState, SourceState, VimState,
};

const DEFAULT_SKILL_PATH: &str = ".agents/skills/ratkit/SKILL.md";

struct SkillPreviewApp {
    widget: MarkdownWidget<'static>,
    source_path: PathBuf,
    show_toc: bool,
    grid_layout: ResizableGrid,
    grid_state: ResizableGridWidgetState,
    grid_area: Rect,
    preview_pane_id: PaneId,
    markdown_area: Rect,
    last_move_processed: Instant,
    toast_message: Option<String>,
    toast_expires_at: Option<Instant>,
}

impl SkillPreviewApp {
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
        .with_has_pane(true)
        .show_toc(show_toc)
        .show_scrollbar(true)
        .show_statusline(true)
    }

    fn new(source_path: PathBuf, source: SourceState) -> Self {
        let show_toc = true;
        let widget = Self::build_widget(source, show_toc);

        let mut grid_layout = ResizableGrid::new(0);
        let preview_pane_id = grid_layout.split_pane_vertically(0).unwrap_or(0);
        let _ = grid_layout.resize_divider(0, 20);

        Self {
            widget,
            source_path,
            show_toc,
            grid_layout,
            grid_state: ResizableGridWidgetState::default(),
            grid_area: Rect::default(),
            preview_pane_id,
            markdown_area: Rect::default(),
            last_move_processed: Instant::now(),
            toast_message: None,
            toast_expires_at: None,
        }
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

    fn update_markdown_area_from_grid(&mut self) {
        let panes = self.grid_layout.get_panes(self.grid_area);
        if let Some(pane) = panes.iter().find(|pane| pane.id == self.preview_pane_id) {
            self.markdown_area = pane.area;
        }
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

                if key.key_code == KeyCode::Char(']') {
                    self.show_toc = !self.show_toc;
                    if let Ok(source) = load_source_from_path(&self.source_path) {
                        self.widget = Self::build_widget(source, self.show_toc);
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
                let was_grid_dragging = self.grid_layout.is_dragging();
                self.handle_grid_mouse(mouse);
                self.update_markdown_area_from_grid();

                if was_grid_dragging || self.grid_layout.is_dragging() {
                    return Ok(CoordinatorAction::Redraw);
                }

                let is_moved = matches!(mouse.kind, crossterm::event::MouseEventKind::Moved);

                if is_moved {
                    if self.last_move_processed.elapsed() < Duration::from_millis(24) {
                        return Ok(CoordinatorAction::Redraw);
                    }
                    self.last_move_processed = Instant::now();
                }

                let mouse_event = CrosstermMouseEvent {
                    kind: mouse.kind,
                    column: mouse.column,
                    row: mouse.row,
                    modifiers: mouse.modifiers,
                };

                let markdown_event = self.widget.handle_mouse(mouse_event, self.markdown_area);
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
                        Ok(CoordinatorAction::Redraw)
                    }
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Tick(_) => {
                if self.clear_expired_toast() {
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
        self.grid_area = frame.area();

        let grid_widget =
            ResizableGridWidget::new(self.grid_layout.clone()).with_state(self.grid_state);
        self.grid_state = grid_widget.state();
        self.grid_layout = grid_widget.layout().clone();
        frame.render_widget(grid_widget, self.grid_area);

        self.update_markdown_area_from_grid();
        if self.markdown_area.width > 0 && self.markdown_area.height > 0 {
            frame.render_widget(&mut self.widget, self.markdown_area);
        }

        if let Some(message) = &self.toast_message {
            if self.markdown_area.height > 0 {
                let toast_width =
                    (message.chars().count() as u16 + 2).min(self.markdown_area.width);
                let toast_area = Rect {
                    x: self.markdown_area.x
                        + self.markdown_area.width.saturating_sub(toast_width) / 2,
                    y: self.markdown_area.y + self.markdown_area.height.saturating_sub(1),
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
}

fn load_source_from_path(path: impl AsRef<Path>) -> io::Result<SourceState> {
    let path = path.as_ref();

    let mut source = SourceState::default();
    source.set_source_file(path)?;
    Ok(source)
}

fn main() -> io::Result<()> {
    let source_path = PathBuf::from(DEFAULT_SKILL_PATH);
    let source = load_source_from_path(&source_path)?;
    let app = SkillPreviewApp::new(source_path, source);
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(250),
        ..RunnerConfig::default()
    };
    run(app, config)
}
