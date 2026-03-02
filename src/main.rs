use std::io;
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
    run, CoordinatorAction, CoordinatorApp, CoordinatorEvent, LayoutResult, RunnerConfig,
};
use ratkit::widgets::markdown_preview::{
    CacheState, CollapseState, DisplaySettings, DoubleClickState, ExpandableState, GitStatsState,
    MarkdownEvent, MarkdownWidget, ScrollState, SelectionState, SourceState, VimState,
};

const DEFAULT_SKILL_PATH: &str = ".agents/skills/ratkit/SKILL.md";

struct SkillPreviewApp {
    widget: MarkdownWidget<'static>,
    markdown_area: Rect,
    last_move_processed: Instant,
    toast_message: Option<String>,
    toast_expires_at: Option<Instant>,
}

impl SkillPreviewApp {
    fn new(source: SourceState) -> Self {
        let markdown_content = source.content().unwrap_or_default().to_owned();

        let mut scroll = ScrollState::default();
        scroll.update_total_lines(markdown_content.lines().count().max(1));

        let widget = MarkdownWidget::new(
            markdown_content,
            scroll,
            source,
            CacheState::default(),
            DisplaySettings::default(),
            CollapseState::default(),
            ExpandableState::default(),
            GitStatsState::default(),
            VimState::default(),
            SelectionState::default(),
            DoubleClickState::default(),
        )
        .with_has_pane(false)
        .show_toc(true)
        .show_scrollbar(true)
        .show_statusline(true);

        Self {
            widget,
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
                        Ok(CoordinatorAction::Continue)
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
        self.markdown_area = frame.area();
        frame.render_widget(&mut self.widget, self.markdown_area);

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

fn load_default_skill_source() -> io::Result<SourceState> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(DEFAULT_SKILL_PATH);

    let mut source = SourceState::default();
    source.set_source_file(path)?;
    Ok(source)
}

fn main() -> io::Result<()> {
    let source = load_default_skill_source()?;
    let app = SkillPreviewApp::new(source);
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(250),
        ..RunnerConfig::default()
    };
    run(app, config)
}
