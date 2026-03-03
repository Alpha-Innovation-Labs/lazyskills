use std::time::Duration;

use ratkit::widgets::markdown_preview::{
    CacheState, CollapseState, DisplaySettings, DoubleClickState, ExpandableState, GitStatsState,
    MarkdownWidget, ScrollState, SelectionState, SourceState, VimState,
};

use crate::app::skills_tree::{
    extract_skill_name_from_frontmatter, fallback_title_from_path, load_source_from_path,
};

use super::state::PreviewState;

pub fn build_widget(source: SourceState, show_toc: bool) -> MarkdownWidget<'static> {
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
    .show_statusline(false)
}

pub fn new_preview_state(
    source_path: std::path::PathBuf,
    source: SourceState,
    show_toc: bool,
) -> PreviewState {
    let preview_title = extract_skill_name_from_frontmatter(&source_path)
        .unwrap_or_else(|| fallback_title_from_path(&source_path));
    let widget = build_widget(source, show_toc);

    PreviewState {
        widget,
        source_path,
        preview_title,
        show_toc,
        show_markdown_pane: true,
        pending_preview_path: None,
        pending_preview_since: None,
    }
}

pub fn queue_open_selected_file(
    preview: &mut PreviewState,
    selected_path: Option<std::path::PathBuf>,
) {
    let Some(selected_path) = selected_path else {
        return;
    };

    if selected_path == preview.source_path {
        return;
    }

    if let Ok(source) = load_source_from_path(&selected_path) {
        preview.source_path = selected_path;
        preview.preview_title = extract_skill_name_from_frontmatter(&preview.source_path)
            .unwrap_or_else(|| fallback_title_from_path(&preview.source_path));
        preview.widget = build_widget(source, preview.show_toc);
    }
}

pub fn open_selected_file_immediate(
    preview: &mut PreviewState,
    selected_path: Option<std::path::PathBuf>,
) {
    let Some(path) = selected_path else {
        return;
    };
    if path == preview.source_path {
        return;
    }
    if let Ok(source) = load_source_from_path(&path) {
        preview.source_path = path;
        preview.preview_title = extract_skill_name_from_frontmatter(&preview.source_path)
            .unwrap_or_else(|| fallback_title_from_path(&preview.source_path));
        preview.widget = build_widget(source, preview.show_toc);
    }
}

pub fn flush_pending_preview_if_ready(preview: &mut PreviewState) -> bool {
    const PREVIEW_DEBOUNCE_MS: u64 = 200;

    let Some(pending_since) = preview.pending_preview_since else {
        return false;
    };
    if pending_since.elapsed() < Duration::from_millis(PREVIEW_DEBOUNCE_MS) {
        return false;
    }

    let Some(selected_path) = preview.pending_preview_path.take() else {
        preview.pending_preview_since = None;
        return false;
    };
    preview.pending_preview_since = None;

    if let Ok(source) = load_source_from_path(&selected_path) {
        preview.source_path = selected_path;
        preview.preview_title = extract_skill_name_from_frontmatter(&preview.source_path)
            .unwrap_or_else(|| fallback_title_from_path(&preview.source_path));
        preview.widget = build_widget(source, preview.show_toc);
        return true;
    }
    false
}

pub fn clear_pending(preview: &mut PreviewState) {
    preview.pending_preview_path = None;
    preview.pending_preview_since = None;
}
