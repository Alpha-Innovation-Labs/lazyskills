use std::path::PathBuf;
use std::time::Instant;

use ratkit::widgets::markdown_preview::MarkdownWidget;

pub struct PreviewState {
    pub widget: MarkdownWidget<'static>,
    pub source_path: PathBuf,
    pub preview_title: String,
    pub show_toc: bool,
    pub show_markdown_pane: bool,
    pub pending_preview_path: Option<PathBuf>,
    pub pending_preview_since: Option<Instant>,
}
