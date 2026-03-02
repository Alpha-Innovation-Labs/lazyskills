use std::path::Path;
use std::time::Instant;

use anyhow::bail;
use crossterm::event::{
    KeyCode, KeyEvent as CrosstermKeyEvent, KeyEventState, MouseEvent as CrosstermMouseEvent,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use ratkit::prelude::{CoordinatorAction, CoordinatorApp, CoordinatorEvent, LayoutResult};
use ratkit::widgets::markdown_preview::{
    CacheState, CollapseState, DisplaySettings, DoubleClickState, ExpandableState, GitStatsState,
    MarkdownEvent, MarkdownWidget, ScrollState, SelectionState, SourceState, VimState,
};

use crate::adapters::skills_repo::{load_skill_docs, SkillDoc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusPane {
    Skills,
    Preview,
}

pub struct SkillsTuiApp {
    docs: Vec<SkillDoc>,
    selected: usize,
    focus: FocusPane,
    markdown_widget: MarkdownWidget<'static>,
    markdown_area: Rect,
    mouse_x: u16,
    mouse_y: u16,
    redraws: u64,
    frames_this_second: u32,
    fps: u16,
    fps_window_start: Instant,
    last_move_processed: Instant,
}

impl SkillsTuiApp {
    pub fn new(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let docs = load_skill_docs(root.as_ref())?;
        if docs.is_empty() {
            bail!("no markdown skills found under {}", root.as_ref().display());
        }

        let markdown_widget = Self::build_markdown_widget(&docs[0].content);

        Ok(Self {
            docs,
            selected: 0,
            focus: FocusPane::Skills,
            markdown_widget,
            markdown_area: Rect::default(),
            mouse_x: 0,
            mouse_y: 0,
            redraws: 0,
            frames_this_second: 0,
            fps: 0,
            fps_window_start: Instant::now(),
            last_move_processed: Instant::now(),
        })
    }

    fn build_markdown_widget(content: &str) -> MarkdownWidget<'static> {
        let text = content.to_owned();
        let mut source = SourceState::default();
        source.set_source_string(text.clone());

        let mut scroll = ScrollState::default();
        scroll.update_total_lines(text.lines().count().max(1));

        MarkdownWidget::new(
            text,
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
        .show_statusline(true)
    }

    fn update_fps(&mut self) {
        self.frames_this_second = self.frames_this_second.saturating_add(1);
        let elapsed = self.fps_window_start.elapsed();
        if elapsed.as_secs() >= 1 {
            let elapsed_ms = elapsed.as_millis().max(1) as u32;
            self.fps = ((self.frames_this_second.saturating_mul(1000)) / elapsed_ms) as u16;
            self.frames_this_second = 0;
            self.fps_window_start = Instant::now();
        }
    }

    fn update_selected(&mut self, next: usize) {
        self.selected = next;
        self.markdown_widget = Self::build_markdown_widget(&self.docs[self.selected].content);
    }

    fn select_next(&mut self) {
        let next = (self.selected + 1) % self.docs.len();
        self.update_selected(next);
    }

    fn select_prev(&mut self) {
        let next = if self.selected == 0 {
            self.docs.len() - 1
        } else {
            self.selected - 1
        };
        self.update_selected(next);
    }
}

impl CoordinatorApp for SkillsTuiApp {
    fn on_event(&mut self, event: CoordinatorEvent) -> LayoutResult<CoordinatorAction> {
        match event {
            CoordinatorEvent::Keyboard(key) => {
                if !key.is_key_down() {
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Tab {
                    self.focus = match self.focus {
                        FocusPane::Skills => FocusPane::Preview,
                        FocusPane::Preview => FocusPane::Skills,
                    };
                    return Ok(CoordinatorAction::Redraw);
                }

                match key.key_code {
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(CoordinatorAction::Quit),
                    KeyCode::Down | KeyCode::Char('j') if self.focus == FocusPane::Skills => {
                        self.select_next();
                        return Ok(CoordinatorAction::Redraw);
                    }
                    KeyCode::Up | KeyCode::Char('k') if self.focus == FocusPane::Skills => {
                        self.select_prev();
                        return Ok(CoordinatorAction::Redraw);
                    }
                    _ => {}
                }

                if self.focus == FocusPane::Skills {
                    return Ok(CoordinatorAction::Continue);
                }

                let key_event = CrosstermKeyEvent {
                    code: key.key_code,
                    modifiers: key.modifiers,
                    kind: key.kind,
                    state: KeyEventState::NONE,
                };

                let markdown_event = self.markdown_widget.handle_key(key_event);
                if matches!(markdown_event, MarkdownEvent::None) {
                    Ok(CoordinatorAction::Continue)
                } else {
                    Ok(CoordinatorAction::Redraw)
                }
            }
            CoordinatorEvent::Mouse(mouse) => {
                let is_moved = matches!(mouse.kind, crossterm::event::MouseEventKind::Moved);
                self.mouse_x = mouse.column;
                self.mouse_y = mouse.row;

                if is_moved {
                    if self.last_move_processed.elapsed().as_millis() < 24 {
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
                    .markdown_widget
                    .handle_mouse(mouse_event, self.markdown_area);
                if is_moved {
                    if matches!(markdown_event, MarkdownEvent::TocHoverChanged { .. }) {
                        Ok(CoordinatorAction::Redraw)
                    } else {
                        Ok(CoordinatorAction::Continue)
                    }
                } else if matches!(markdown_event, MarkdownEvent::None) {
                    Ok(CoordinatorAction::Continue)
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
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        let active_path = self.docs[self.selected].relative_path.as_str();
        let focus_hint = match self.focus {
            FocusPane::Skills => "focus: skills",
            FocusPane::Preview => "focus: preview",
        };
        let title_text = format!(
            "skills-tui | {} | {} fps | redraws {} | mouse {},{} | tab switch pane | {} | q quit",
            active_path, self.fps, self.redraws, self.mouse_x, self.mouse_y, focus_hint
        );
        let title = Paragraph::new(title_text).style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(title, rows[0]);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
            .split(rows[1]);

        let items: Vec<ListItem> = self
            .docs
            .iter()
            .map(|doc| ListItem::new(doc.title.clone()))
            .collect();

        let nav_list = List::new(items)
            .block(
                Block::default()
                    .title("Skills")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if self.focus == FocusPane::Skills {
                        Color::Yellow
                    } else {
                        Color::Cyan
                    })),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut list_state = ListState::default();
        list_state.select(Some(self.selected));
        frame.render_stateful_widget(nav_list, columns[0], &mut list_state);

        self.markdown_area = columns[1];
        let preview_border = Block::default()
            .title("Preview")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if self.focus == FocusPane::Preview {
                Color::Yellow
            } else {
                Color::Cyan
            }));
        let preview_area = preview_border.inner(self.markdown_area);
        frame.render_widget(preview_border, self.markdown_area);
        self.markdown_area = preview_area;
        frame.render_widget(&mut self.markdown_widget, self.markdown_area);
    }
}
