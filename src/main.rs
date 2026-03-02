use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{
    KeyCode, KeyEvent as CrosstermKeyEvent, KeyEventState, MouseEvent as CrosstermMouseEvent,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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
    mouse_x: u16,
    mouse_y: u16,
    redraws: u64,
    frames_this_second: u32,
    fps: u16,
    fps_window_start: Instant,
    last_move_processed: Instant,
}

impl SkillPreviewApp {
    fn new(markdown_content: String) -> Self {
        let mut source = SourceState::default();
        source.set_source_string(markdown_content.clone());

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
            mouse_x: 0,
            mouse_y: 0,
            redraws: 0,
            frames_this_second: 0,
            fps: 0,
            fps_window_start: Instant::now(),
            last_move_processed: Instant::now(),
        }
    }

    fn update_fps(&mut self) {
        self.frames_this_second = self.frames_this_second.saturating_add(1);
        let elapsed = self.fps_window_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let elapsed_ms = elapsed.as_millis().max(1) as u32;
            self.fps = ((self.frames_this_second.saturating_mul(1000)) / elapsed_ms) as u16;
            self.frames_this_second = 0;
            self.fps_window_start = Instant::now();
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

                let key_event = CrosstermKeyEvent {
                    code: key.key_code,
                    modifiers: key.modifiers,
                    kind: key.kind,
                    state: KeyEventState::NONE,
                };

                let markdown_event = self.widget.handle_key(key_event);
                if matches!(markdown_event, MarkdownEvent::None) {
                    Ok(CoordinatorAction::Continue)
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Mouse(mouse) => {
                let is_moved = matches!(mouse.kind, crossterm::event::MouseEventKind::Moved);
                self.mouse_x = mouse.x();
                self.mouse_y = mouse.y();

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
            CoordinatorEvent::Resize(_) => Ok(CoordinatorAction::Redraw),
            _ => Ok(CoordinatorAction::Continue),
        }
    }

    fn on_draw(&mut self, frame: &mut Frame) {
        self.redraws = self.redraws.saturating_add(1);
        self.update_fps();

        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        let dev_bar_area = chunks[0];
        let markdown_area = chunks[1];
        self.markdown_area = markdown_area;

        let dev_text = format!(
            " PREVIEW | FPS {:>3} | REDRAWS {:>7} | MOUSE {:>4},{:<4} | {} | q quit | wheel scroll ",
            self.fps, self.redraws, self.mouse_x, self.mouse_y, DEFAULT_SKILL_PATH
        );
        frame.render_widget(
            Paragraph::new(Line::from(dev_text))
                .style(Style::default().fg(Color::Black).bg(Color::Cyan)),
            dev_bar_area,
        );

        frame.render_widget(&mut self.widget, markdown_area);
    }
}

fn load_default_skill_markdown() -> io::Result<String> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(DEFAULT_SKILL_PATH);
    fs::read_to_string(path)
}

fn main() -> io::Result<()> {
    let markdown = load_default_skill_markdown()?;
    let app = SkillPreviewApp::new(markdown);
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(250),
        ..RunnerConfig::default()
    };
    run(app, config)
}
