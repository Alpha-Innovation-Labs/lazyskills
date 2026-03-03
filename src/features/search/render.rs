use lazyskills::config::FavoriteSkill;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use ratkit::primitives::pane::Pane;

use crate::app::skills_tree::SkillTreeNode;
use crate::features::{
    detail::render::{render_skill_detail_pane, SkillDetailPaneData},
    favorites::logic as favorites_logic,
    search::{logic as search_logic, state::SearchState},
};

const CONSOLE_BG: Color = Color::Rgb(10, 14, 20);
const CONSOLE_TEXT: Color = Color::Rgb(217, 225, 234);
const CONSOLE_MUTED: Color = Color::Rgb(122, 134, 148);
const CONSOLE_BORDER: Color = Color::Rgb(42, 51, 64);
const CONSOLE_ACCENT: Color = Color::Rgb(53, 194, 255);
const CONSOLE_SUCCESS: Color = Color::Rgb(95, 211, 141);
const CONSOLE_WARNING: Color = Color::Rgb(246, 193, 119);
const CONSOLE_ERROR: Color = Color::Rgb(247, 118, 142);
const CONSOLE_FAVORITE: Color = Color::Rgb(255, 209, 102);

fn truncate_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let char_count = value.chars().count();
    if char_count <= width {
        return format!("{:<width$}", value, width = width);
    }

    if width <= 3 {
        return value.chars().take(width).collect::<String>();
    }

    let mut output = value.chars().take(width - 3).collect::<String>();
    output.push_str("...");
    output
}

fn format_installs_compact(value: u64) -> String {
    if value >= 1_000_000_000 {
        return format!("{}b", value / 1_000_000_000);
    }
    if value >= 1_000_000 {
        return format!("{}m", value / 1_000_000);
    }
    if value >= 1_000 {
        return format!("{}k", value / 1_000);
    }
    value.to_string()
}

struct ColumnLayout {
    fav_w: usize,
    pro_w: usize,
    glo_w: usize,
    name_w: usize,
    creator_w: usize,
    gh_w: usize,
    installs_w: usize,
}

fn compute_column_widths(width: u16) -> ColumnLayout {
    let width_usize = width as usize;
    let fav_w = 3usize;
    let pro_w = 3usize;
    let glo_w = 3usize;
    let name_w = 30usize;
    let gh_w = 10usize;
    let installs_w = 8usize;
    let separators = 6usize;

    let fixed = fav_w + pro_w + glo_w + name_w + gh_w + installs_w + separators;
    let creator_w = if width_usize > fixed {
        width_usize - fixed
    } else {
        8
    };

    ColumnLayout {
        fav_w,
        pro_w,
        glo_w,
        name_w,
        creator_w,
        gh_w,
        installs_w,
    }
}

fn is_slug_installed_in_nodes(slug: &str, nodes: &[SkillTreeNode]) -> bool {
    for node in nodes {
        let Some(metadata) = favorites_logic::favorite_for_node(node) else {
            if is_slug_installed_in_nodes(slug, &node.children) {
                return true;
            }
            continue;
        };
        if favorites_logic::favorite_matches_search_slug(&metadata, slug) {
            return true;
        }
        if is_slug_installed_in_nodes(slug, &node.children) {
            return true;
        }
    }
    false
}

pub fn render_search_view(
    frame: &mut Frame,
    tree_area: Rect,
    markdown_area: Rect,
    search: &mut SearchState,
    favorites: &[FavoriteSkill],
    project_nodes: &[SkillTreeNode],
    global_nodes: &[SkillTreeNode],
    focus_tree: bool,
    focus_preview: bool,
    terminal_icon: &str,
    _selected_installed: bool,
) {
    let left_border = if focus_tree {
        CONSOLE_ACCENT
    } else {
        CONSOLE_BORDER
    };
    let right_border = if focus_preview {
        CONSOLE_ACCENT
    } else {
        CONSOLE_BORDER
    };

    let left_pane = Pane::new("Search")
        .with_icon(terminal_icon)
        .border_style(Style::default().fg(left_border));
    let (left_inner, _) = left_pane.render_block(frame, tree_area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(CONSOLE_BG)),
        left_inner,
    );

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(left_inner);

    let total_count = search.search_results.len();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Search", Style::default().fg(CONSOLE_TEXT)),
            Span::styled(
                format!("  Total: {} | Filtered: {}", total_count, total_count),
                Style::default().fg(CONSOLE_MUTED),
            ),
        ])),
        left_rows[0],
    );

    let query_prefix = "Query: ";
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(query_prefix, Style::default().fg(CONSOLE_ACCENT)),
            if search.search_query.is_empty() {
                Span::styled("Press `/` to search", Style::default().fg(CONSOLE_MUTED))
            } else {
                Span::styled(
                    search.search_query.clone(),
                    Style::default().fg(CONSOLE_TEXT),
                )
            },
        ])),
        left_rows[1],
    );

    if search.input_focused {
        let cursor_x = left_rows[1]
            .x
            .saturating_add(
                (query_prefix.chars().count() + search.search_query.chars().count()) as u16,
            )
            .min(
                left_rows[1]
                    .x
                    .saturating_add(left_rows[1].width.saturating_sub(1)),
            );
        frame.set_cursor_position((cursor_x, left_rows[1].y));
    }

    let layout = compute_column_widths(left_rows[3].width);
    let fav_w = layout.fav_w;
    let pro_w = layout.pro_w;
    let glo_w = layout.glo_w;
    let name_w = layout.name_w;
    let creator_w = layout.creator_w;
    frame.render_widget(
        Paragraph::new(Line::from({
            let mut spans = vec![
                Span::styled(
                    truncate_cell("Fav", fav_w),
                    Style::default().fg(CONSOLE_FAVORITE),
                ),
                Span::styled(" ", Style::default().fg(CONSOLE_BORDER)),
                Span::styled(
                    truncate_cell("Pro", pro_w),
                    Style::default().fg(CONSOLE_ACCENT),
                ),
                Span::styled(" ", Style::default().fg(CONSOLE_BORDER)),
                Span::styled(
                    truncate_cell("Glo", glo_w),
                    Style::default().fg(CONSOLE_ACCENT),
                ),
                Span::styled(" ", Style::default().fg(CONSOLE_BORDER)),
                Span::styled(
                    truncate_cell("Name", name_w),
                    Style::default().fg(CONSOLE_ACCENT),
                ),
                Span::styled(" ", Style::default().fg(CONSOLE_BORDER)),
                Span::styled(
                    truncate_cell("Creator", creator_w),
                    Style::default().fg(CONSOLE_ACCENT),
                ),
            ];

            spans.push(Span::styled(" ", Style::default().fg(CONSOLE_BORDER)));
            spans.push(Span::styled(
                truncate_cell("GH", layout.gh_w),
                Style::default().fg(CONSOLE_ACCENT),
            ));
            spans.push(Span::styled(" ", Style::default().fg(CONSOLE_BORDER)));
            spans.push(Span::styled(
                truncate_cell("Installs", layout.installs_w),
                Style::default().fg(CONSOLE_ACCENT),
            ));
            spans
        })),
        left_rows[2],
    );

    let table_height = left_rows[3].height as usize;
    if search.search_selected < search.search_offset {
        search.search_offset = search.search_selected;
    }
    if table_height > 0
        && search.search_selected >= search.search_offset.saturating_add(table_height)
    {
        search.search_offset = search
            .search_selected
            .saturating_sub(table_height.saturating_sub(1));
    }

    let mut table_lines = Vec::new();
    for (idx, item) in search
        .search_results
        .iter()
        .enumerate()
        .skip(search.search_offset)
        .take(table_height)
    {
        let slug = search_logic::skill_slug(item).unwrap_or_default();
        let is_favorite = favorites
            .iter()
            .any(|entry| favorites_logic::favorite_matches_search_slug(entry, &slug));
        let installed_project = is_slug_installed_in_nodes(&slug, project_nodes);
        let installed_global = is_slug_installed_in_nodes(&slug, global_nodes);
        let creator = if item.source.is_empty() {
            "unknown".to_string()
        } else {
            item.source.clone()
        };
        let installs = format_installs_compact(item.installs);
        let installs_color = CONSOLE_TEXT;
        let row_is_selected = idx == search.search_selected;
        let gh_from_cache = search
            .search_gh_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&slug).cloned());
        let gh_value = if let Some(stars) = gh_from_cache {
            format!("★ {}", stars)
        } else if row_is_selected {
            search
                .search_detail
                .as_ref()
                .map(|detail| format!("★ {}", detail.github_stars))
                .unwrap_or_else(|| "★ -".to_string())
        } else {
            "★ -".to_string()
        };

        let row_style = if row_is_selected && is_favorite {
            Style::default().fg(CONSOLE_BG).bg(CONSOLE_FAVORITE)
        } else if row_is_selected {
            Style::default().fg(CONSOLE_BG).bg(CONSOLE_ACCENT)
        } else {
            Style::default()
                .fg(if is_favorite {
                    CONSOLE_FAVORITE
                } else {
                    CONSOLE_TEXT
                })
                .bg(CONSOLE_BG)
        };
        let unfocused_row_fg = if is_favorite {
            CONSOLE_FAVORITE
        } else {
            CONSOLE_TEXT
        };

        let mut line_spans = vec![
            Span::styled(
                truncate_cell(if is_favorite { "★" } else { " " }, fav_w),
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else {
                    CONSOLE_FAVORITE
                }),
            ),
            Span::styled(
                " ",
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else {
                    unfocused_row_fg
                }),
            ),
            Span::styled(
                truncate_cell(if installed_project { "●" } else { "○" }, pro_w),
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else if installed_project {
                    CONSOLE_FAVORITE
                } else {
                    unfocused_row_fg
                }),
            ),
            Span::styled(
                " ",
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else {
                    unfocused_row_fg
                }),
            ),
            Span::styled(
                truncate_cell(if installed_global { "●" } else { "○" }, glo_w),
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else if installed_global {
                    CONSOLE_FAVORITE
                } else {
                    unfocused_row_fg
                }),
            ),
            Span::styled(
                " ",
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else {
                    unfocused_row_fg
                }),
            ),
            Span::styled(
                truncate_cell(&item.name, name_w),
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else if is_favorite {
                    CONSOLE_FAVORITE
                } else {
                    CONSOLE_TEXT
                }),
            ),
            Span::styled(
                " ",
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else {
                    unfocused_row_fg
                }),
            ),
            Span::styled(
                truncate_cell(&creator, creator_w),
                row_style.fg(if row_is_selected {
                    CONSOLE_BG
                } else if is_favorite {
                    CONSOLE_FAVORITE
                } else {
                    CONSOLE_MUTED
                }),
            ),
        ];
        line_spans.push(Span::styled(
            " ",
            row_style.fg(if row_is_selected {
                CONSOLE_BG
            } else {
                unfocused_row_fg
            }),
        ));
        line_spans.push(Span::styled(
            truncate_cell(&gh_value, layout.gh_w),
            row_style.fg(if row_is_selected {
                CONSOLE_BG
            } else {
                CONSOLE_FAVORITE
            }),
        ));
        line_spans.push(Span::styled(
            " ",
            row_style.fg(if row_is_selected {
                CONSOLE_BG
            } else {
                unfocused_row_fg
            }),
        ));
        line_spans.push(Span::styled(
            truncate_cell(&installs, layout.installs_w),
            row_style.fg(if row_is_selected {
                CONSOLE_BG
            } else if is_favorite {
                CONSOLE_FAVORITE
            } else {
                installs_color
            }),
        ));
        table_lines.push(Line::from(line_spans));
    }

    if table_lines.is_empty() {
        table_lines.push(Line::from(Span::styled(
            "No results. Type a query or press Ctrl+R.",
            Style::default().fg(CONSOLE_MUTED),
        )));
    }
    frame.render_widget(
        Paragraph::new(table_lines).style(Style::default().bg(CONSOLE_BG)),
        left_rows[3],
    );

    let status_color =
        if search.search_status.contains("failed") || search.search_status.contains("invalid") {
            CONSOLE_ERROR
        } else if search.search_status.starts_with("Installed")
            || search.search_status.starts_with("Updated")
            || search.search_status.starts_with("Removed")
        {
            CONSOLE_SUCCESS
        } else if search.search_status.contains("skipped") {
            CONSOLE_WARNING
        } else {
            CONSOLE_MUTED
        };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            search.search_status.clone(),
            Style::default().fg(status_color),
        )))
        .style(Style::default().bg(CONSOLE_BG)),
        left_rows[4],
    );

    render_skill_detail_pane(
        frame,
        markdown_area,
        terminal_icon,
        right_border,
        SkillDetailPaneData {
            detail: search.search_detail.as_ref(),
            empty_line_1: if search.search_results.is_empty() {
                "No detail for selected skill."
            } else {
                "Select a skill to view details."
            },
            empty_line_2: if search.search_results.is_empty() {
                "Search skills and select one from the list."
            } else {
                "Use Up/Down to navigate."
            },
        },
    );
}
