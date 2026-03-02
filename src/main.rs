use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

mod app;
mod features;

use crossterm::event::{
    KeyCode, KeyEvent as CrosstermKeyEvent, KeyEventState, MouseButton,
    MouseEvent as CrosstermMouseEvent, MouseEventKind,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use ratkit::prelude::{
    run, CoordinatorAction, CoordinatorApp, CoordinatorEvent, LayoutResult, RunnerConfig,
};
use ratkit::primitives::dialog::{DialogActionsLayout, DialogShadow, DialogWrap};
use ratkit::primitives::menu_bar::{MenuBar, MenuItem};
use ratkit::primitives::pane::Pane;
use ratkit::widgets::markdown_preview::{MarkdownEvent, SourceState};
use ratkit::widgets::{Dialog, DialogWidget};
use ratkit::widgets::{HotkeyFooter, HotkeyItem};

use crate::app::skills_tree::{
    collect_expanded_skill_paths, first_skill_file, load_global_skill_hierarchy,
    load_project_skill_hierarchy, load_source_from_path, preview_relative_to_skills,
    skill_node_at_path, skill_remove_target_from_path, SkillTreeNode, DEFAULT_SKILL_PATH,
};
use crate::features::detail::render::{render_skill_detail_pane, SkillDetailPaneData};
use crate::features::{
    config::{logic as config_logic, state::ConfigState},
    delete_confirm::{logic as delete_confirm_logic, state::DeleteConfirmDialogState},
    detail::{logic as detail_logic, state::DetailState},
    favorites::{logic as favorites_logic, state::FavoritesState},
    preview::{logic as preview_logic, state::PreviewState},
    search::{logic as search_logic, render as search_render, state::SearchState},
    startup_dialog::{logic as startup_dialog_logic, state::StartupDialogState},
    tree_nav::{
        logic as tree_nav_logic,
        state::{ExpandedSkillPaths, SkillPath},
    },
};
use skills_tui::config::{
    initialize_skills_command_config as initialize_app_config, load_user_config,
    persist_user_config, AppConfig, FavoriteSkill, UserConfig, APP_CONFIG_PATH,
};
use skills_tui::services::skills_command::{
    install_skill_from_slug_global, install_skill_from_slug_with_agents,
    patch_project_lock_after_remove, remove_skill_noninteractive,
    remove_skill_noninteractive_scoped, run_configured_skills_command,
};

const TERMINAL_ICON: &str = "";
const YAZI_CYAN: Color = Color::Rgb(3, 169, 244);
const UNFOCUSED_PANE_BORDER: Color = Color::Rgb(42, 51, 64);
const FAVORITE_ORANGE: Color = Color::Rgb(255, 209, 102);
const SUPPORTED_AGENTS: &[&str] = &[
    "adal",
    "amp",
    "antigravity",
    "augment",
    "claude-code",
    "cline",
    "codebuddy",
    "codex",
    "command-code",
    "continue",
    "cortex",
    "crush",
    "cursor",
    "droid",
    "gemini-cli",
    "github-copilot",
    "goose",
    "iflow-cli",
    "junie",
    "kilo",
    "kimi-cli",
    "kiro-cli",
    "kode",
    "mcpjam",
    "mistral-vibe",
    "mux",
    "neovate",
    "opencode",
    "openclaw",
    "openhands",
    "pi",
    "pochi",
    "qoder",
    "qwen-code",
    "replit",
    "roo",
    "trae",
    "trae-cn",
    "universal",
    "windsurf",
    "zencoder",
];

fn truncate_to_width(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return value.chars().take(width).collect::<String>();
    }
    let mut out = value.chars().take(width - 3).collect::<String>();
    out.push_str("...");
    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusPane {
    Tree,
    Preview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppView {
    Project,
    Search,
    Global,
    Favorites,
    Config,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallOrigin {
    Search,
    Favorites,
}

#[derive(Debug)]
enum InstallModalPhase {
    Installing,
    Finished { success: bool, message: String },
}

struct InstallModalState {
    slug: String,
    scope_label: String,
    origin: InstallOrigin,
    phase: InstallModalPhase,
    receiver: Option<Receiver<Result<String, String>>>,
}

struct AgentPickerModalState {
    query: String,
    selected_agents: Vec<String>,
    selected_index: usize,
    required_on_startup: bool,
}

struct SkillPreviewApp {
    app_config: AppConfig,
    startup_dialog: Option<StartupDialogState>,
    preview: PreviewState,
    menu: MenuBar,
    project_skills_nodes: Vec<SkillTreeNode>,
    global_skills_nodes: Vec<SkillTreeNode>,
    favorites: FavoritesState,
    skills_selected_path: Option<SkillPath>,
    skills_expanded: ExpandedSkillPaths,
    skills_offset: usize,
    current_view: AppView,
    focus: FocusPane,
    navbar_area: Rect,
    grid_area: Rect,
    tree_area: Rect,
    tree_content_area: Rect,
    markdown_area: Rect,
    detail_area: Rect,
    markdown_inner_area: Rect,
    last_move_processed: Instant,
    toast_message: Option<String>,
    toast_expires_at: Option<Instant>,
    show_hotkeys_modal: bool,
    delete_confirm_dialog: Option<DeleteConfirmDialogState>,
    install_modal: Option<InstallModalState>,
    agent_picker_modal: Option<AgentPickerModalState>,
    pending_startup_agent_picker: bool,
    detail: DetailState,
    search: SearchState,
    config: ConfigState,
}

impl SkillPreviewApp {
    fn build_menu() -> MenuBar {
        MenuBar::new(vec![
            MenuItem::new("Project [1]", 0),
            MenuItem::new("Global [2]", 1),
            MenuItem::new("Search [3]", 2),
            MenuItem::new("Favorites [4]", 3),
            MenuItem::new("Config [5]", 4),
        ])
        .with_selected(0)
    }

    fn set_menu_selection(&mut self, target: usize) {
        for (idx, item) in self.menu.items.iter_mut().enumerate() {
            item.selected = idx == target;
        }
    }

    fn first_selectable_skill_path(nodes: &[SkillTreeNode]) -> Option<Vec<usize>> {
        fn walk(nodes: &[SkillTreeNode], base: &mut Vec<usize>) -> Option<Vec<usize>> {
            for (idx, node) in nodes.iter().enumerate() {
                base.push(idx);
                if node.skill_file.is_some() {
                    return Some(base.clone());
                }
                if let Some(path) = walk(&node.children, base) {
                    return Some(path);
                }
                let _ = base.pop();
            }
            None
        }

        walk(nodes, &mut Vec::new())
    }

    fn reset_selection_to_first_skill(&mut self) {
        let nodes = self.active_skills_nodes();
        self.skills_selected_path =
            Self::first_selectable_skill_path(nodes).or_else(|| nodes.first().map(|_| vec![0]));
    }

    fn toggle_project_global_detail_pane(&mut self) {
        let show_detail_pane = detail_logic::toggle_project_global_detail_pane(&mut self.detail);
        self.markdown_area = Rect::default();
        self.detail_area = Rect::default();

        if let Err(err) = self.persist_favorites() {
            self.show_toast(format!("Failed to save UI settings: {}", err));
        }

        if show_detail_pane {
            self.queue_project_global_detail_refresh();
        }
    }

    fn toggle_project_global_markdown_pane(&mut self) {
        self.preview.show_markdown_pane = !self.preview.show_markdown_pane;
        self.markdown_area = Rect::default();
        self.markdown_inner_area = Rect::default();
        self.detail_area = Rect::default();

        if let Err(err) = self.persist_favorites() {
            self.show_toast(format!("Failed to save UI settings: {}", err));
        }
    }

    fn set_view(&mut self, view: AppView) {
        self.current_view = view;
        match view {
            AppView::Project => self.set_menu_selection(0),
            AppView::Global => self.set_menu_selection(1),
            AppView::Search => self.set_menu_selection(2),
            AppView::Favorites => self.set_menu_selection(3),
            AppView::Config => self.set_menu_selection(4),
        }

        if matches!(
            view,
            AppView::Project | AppView::Global | AppView::Favorites
        ) {
            self.markdown_area = Rect::default();
            self.markdown_inner_area = Rect::default();
            self.detail_area = Rect::default();

            self.skills_offset = 0;
            self.reset_selection_to_first_skill();
            preview_logic::clear_pending(&mut self.preview);
            self.ensure_skill_selection_visible();
            self.open_selected_file_immediate();
            if self.detail.show_detail_pane
                && matches!(
                    view,
                    AppView::Project | AppView::Global | AppView::Favorites
                )
            {
                self.refresh_project_global_detail_now();
            }
        } else if matches!(view, AppView::Search) {
            self.detail_area = Rect::default();
            self.search.input_focused = false;
            self.search.search_status = "Press / to focus search input".to_string();
            if self.search.search_results.is_empty() {
                self.refresh_search_results();
            }
        } else if matches!(view, AppView::Config) {
            self.config.status = "Edit values. Ctrl+S save. Up/Down select field.".to_string();
        }
    }

    fn refresh_search_results(&mut self) {
        search_logic::refresh_search_results(&mut self.search);
    }

    fn queue_search_refresh(&mut self) {
        search_logic::queue_search_refresh(&mut self.search);
    }

    fn flush_pending_search_refresh_if_ready(&mut self) -> bool {
        search_logic::flush_pending_search_refresh_if_ready(&mut self.search)
    }

    fn queue_selected_search_detail(&mut self) {
        search_logic::queue_selected_search_detail(&mut self.search);
    }

    fn begin_install_modal(
        &mut self,
        slug: String,
        scope_label: &str,
        global_install: bool,
        origin: InstallOrigin,
    ) {
        let (tx, rx) = mpsc::channel::<Result<String, String>>();
        let skills_config = self.app_config.skills_command.clone();
        let default_agents = self.app_config.skills_command.default_agents.clone();
        let slug_for_worker = slug.clone();
        thread::spawn(move || {
            let result = if global_install {
                install_skill_from_slug_global(&skills_config, &slug_for_worker)
            } else {
                install_skill_from_slug_with_agents(
                    &skills_config,
                    &slug_for_worker,
                    &default_agents,
                )
            };
            let _ = tx.send(result);
        });

        self.install_modal = Some(InstallModalState {
            slug,
            scope_label: scope_label.to_string(),
            origin,
            phase: InstallModalPhase::Installing,
            receiver: Some(rx),
        });
    }

    fn poll_install_modal_progress(&mut self) -> bool {
        let Some(modal) = self.install_modal.as_mut() else {
            return false;
        };
        if !matches!(modal.phase, InstallModalPhase::Installing) {
            return false;
        }
        let Some(receiver) = modal.receiver.as_ref() else {
            return false;
        };

        let slug = modal.slug.clone();
        let scope_label = modal.scope_label.clone();
        let origin = modal.origin;

        match receiver.try_recv() {
            Ok(result) => {
                modal.receiver = None;

                let (success, msg) = match result {
                    Ok(_) => (true, format!("Installed {} to {}", slug, scope_label)),
                    Err(err) => (false, format!("Install failed for {}: {}", slug, err)),
                };

                if success {
                    self.refresh_skill_hierarchies();
                    self.show_toast(msg.clone());
                }
                if matches!(origin, InstallOrigin::Search) {
                    self.search.search_status = msg.clone();
                }
                if let Some(modal) = self.install_modal.as_mut() {
                    modal.phase = InstallModalPhase::Finished {
                        success,
                        message: msg,
                    };
                }
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                modal.receiver = None;
                if let Some(modal) = self.install_modal.as_mut() {
                    modal.phase = InstallModalPhase::Finished {
                        success: false,
                        message: format!("Install failed for {}: worker disconnected", slug),
                    };
                }
                true
            }
        }
    }

    fn open_agent_picker_modal(&mut self, required_on_startup: bool) {
        let mut selected = self.app_config.skills_command.default_agents.clone();
        selected.sort();
        selected.dedup();
        self.agent_picker_modal = Some(AgentPickerModalState {
            query: String::new(),
            selected_agents: selected,
            selected_index: 0,
            required_on_startup,
        });
    }

    fn filtered_agent_keys(query: &str) -> Vec<&'static str> {
        let q = query.trim().to_ascii_lowercase();
        let mut items = SUPPORTED_AGENTS
            .iter()
            .copied()
            .filter(|agent| q.is_empty() || agent.to_ascii_lowercase().contains(&q))
            .collect::<Vec<_>>();
        items.sort();
        items
    }

    fn save_agent_picker_selection(&mut self) {
        let Some(modal) = self.agent_picker_modal.as_ref() else {
            return;
        };
        let mut selected = modal.selected_agents.clone();
        selected.sort();
        selected.dedup();
        self.app_config.skills_command.default_agents = selected.clone();
        match skills_tui::config::persist_app_config(&self.app_config) {
            Ok(_) => {
                self.pending_startup_agent_picker = false;
                self.show_toast(format!(
                    "Saved {} default agent(s)",
                    self.app_config.skills_command.default_agents.len()
                ));
            }
            Err(err) => {
                self.show_toast(format!("Failed to save default agents: {err}"));
            }
        }
    }

    fn install_selected_search_skill(&mut self) {
        let slug = match search_logic::install_selected_search_skill_slug(&self.search) {
            Ok(slug) => slug,
            Err(message) => {
                self.search.search_status = message;
                return;
            }
        };
        self.begin_install_modal(slug, "project (local)", false, InstallOrigin::Search);
    }

    fn selected_search_slug(&self) -> Result<String, String> {
        search_logic::install_selected_search_skill_slug(&self.search)
    }

    fn search_skill_name_from_slug(slug: &str) -> &str {
        slug.rsplit('/').next().unwrap_or(slug)
    }

    fn node_matches_search_slug(node: &SkillTreeNode, slug: &str) -> bool {
        let Some(metadata) = favorites_logic::favorite_for_node(node) else {
            return false;
        };
        favorites_logic::favorite_matches_search_slug(&metadata, slug)
    }

    fn find_skill_path_for_search_slug(
        nodes: &[SkillTreeNode],
        slug: &str,
        path: &mut Vec<usize>,
    ) -> Option<SkillPath> {
        for (idx, node) in nodes.iter().enumerate() {
            path.push(idx);
            if Self::node_matches_search_slug(node, slug) {
                return Some(path.clone());
            }
            if let Some(found) = Self::find_skill_path_for_search_slug(&node.children, slug, path) {
                return Some(found);
            }
            let _ = path.pop();
        }
        None
    }

    fn find_installed_search_target(&self, slug: &str) -> Option<(AppView, SkillPath)> {
        if let Some(path) =
            Self::find_skill_path_for_search_slug(&self.project_skills_nodes, slug, &mut Vec::new())
        {
            return Some((AppView::Project, path));
        }
        if let Some(path) =
            Self::find_skill_path_for_search_slug(&self.global_skills_nodes, slug, &mut Vec::new())
        {
            return Some((AppView::Global, path));
        }
        None
    }

    fn is_search_skill_installed(&self, slug: &str) -> bool {
        self.find_installed_search_target(slug).is_some()
    }

    fn open_installed_search_preview(&mut self, slug: &str) -> bool {
        let Some((view, path)) = self.find_installed_search_target(slug) else {
            return false;
        };

        self.set_view(view);
        self.skills_selected_path = Some(path);
        self.ensure_skill_selection_visible();
        self.open_selected_file_immediate();
        self.focus = FocusPane::Preview;
        if self.detail.show_detail_pane
            && matches!(
                self.current_view,
                AppView::Project | AppView::Global | AppView::Favorites
            )
        {
            self.refresh_project_global_detail_now();
        }
        true
    }

    fn update_selected_search_skill(&mut self) {
        let slug = match self.selected_search_slug() {
            Ok(slug) => slug,
            Err(message) => {
                self.search.search_status = message;
                return;
            }
        };

        if !self.is_search_skill_installed(&slug) {
            self.search.search_status = "Update skipped: skill is not installed".to_string();
            return;
        }

        match run_configured_skills_command(&self.app_config.skills_command, &["add", &slug]) {
            Ok(_) => {
                self.refresh_skill_hierarchies();
                self.show_toast(format!("Updated {slug}"));
                self.search.search_status = format!("Updated {slug}");
            }
            Err(err) => {
                self.search.search_status = format!("Update failed: {err}");
            }
        }
    }

    fn remove_selected_search_skill(&mut self) {
        let slug = match self.selected_search_slug() {
            Ok(slug) => slug,
            Err(message) => {
                self.search.search_status = message;
                return;
            }
        };

        if !self.is_search_skill_installed(&slug) {
            self.search.search_status = "Remove skipped: skill is not installed".to_string();
            return;
        }

        let fallback = Self::search_skill_name_from_slug(&slug).to_string();
        let removed = remove_skill_noninteractive(&self.app_config.skills_command, &slug)
            .or_else(|_| remove_skill_noninteractive(&self.app_config.skills_command, &fallback));
        match removed {
            Ok(_) => {
                self.refresh_skill_hierarchies();
                self.search.search_status = format!("Removed {slug}");
                self.show_toast(format!("Removed {slug}"));
            }
            Err(err) => {
                self.search.search_status = format!("Remove failed: {err}");
            }
        }
    }

    fn toggle_selected_search_favorite(&mut self) {
        let slug = match search_logic::install_selected_search_skill_slug(&self.search) {
            Ok(slug) => slug,
            Err(message) => {
                self.search.search_status = message;
                return;
            }
        };

        let (install_skill, source) =
            if let Some((owner, repo, skill)) = search_logic::split_slug(&slug) {
                (skill.to_string(), Some(format!("{owner}/{repo}")))
            } else {
                (slug.clone(), None)
            };

        self.toggle_favorite_slug(FavoriteSkill {
            display_slug: install_skill.clone(),
            install_skill,
            source,
            source_type: Some("github".to_string()),
        });
    }

    fn flush_pending_search_detail_if_ready(&mut self) -> bool {
        search_logic::flush_pending_search_detail_if_ready(&mut self.search)
    }

    fn queue_project_global_detail_refresh(&mut self) {
        let slug = if matches!(
            self.current_view,
            AppView::Project | AppView::Global | AppView::Favorites
        ) {
            self.selected_skill_identity().and_then(|(_, favorite, _)| {
                detail_logic::selected_project_global_slug(&favorite)
                    .or_else(|| self.resolve_global_slug_via_search(&favorite))
            })
        } else {
            None
        };
        detail_logic::queue_project_global_detail_refresh(&mut self.detail, slug);
    }

    fn refresh_project_global_detail_now(&mut self) {
        let slug = if matches!(
            self.current_view,
            AppView::Project | AppView::Global | AppView::Favorites
        ) {
            self.selected_skill_identity().and_then(|(_, favorite, _)| {
                detail_logic::selected_project_global_slug(&favorite)
                    .or_else(|| self.resolve_global_slug_via_search(&favorite))
            })
        } else {
            None
        };

        let _ = detail_logic::fetch_project_global_detail_now(
            &mut self.detail,
            self.search.search_client.as_ref(),
            slug.as_deref(),
        );
    }

    fn resolve_global_slug_via_search(&self, favorite: &FavoriteSkill) -> Option<String> {
        let client = self.search.search_client.as_ref()?;
        let skill_name = favorite.install_skill.trim();
        if skill_name.is_empty() {
            return None;
        }

        let response = client.fetch_search_cached_swr(skill_name, 25).ok()?;
        let exact = response
            .skills
            .iter()
            .filter(|item| {
                item.skill_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(skill_name))
                    .unwrap_or(false)
            })
            .max_by_key(|item| item.installs);

        let candidate = exact.or_else(|| {
            response
                .skills
                .iter()
                .filter(|item| {
                    item.id
                        .as_ref()
                        .map(|id| id.ends_with(&format!("/{skill_name}")))
                        .unwrap_or(false)
                })
                .max_by_key(|item| item.installs)
        })?;

        if let Some(id) = &candidate.id {
            if !id.is_empty() {
                return Some(id.clone());
            }
        }

        let skill_id = candidate.skill_id.as_ref()?;
        if candidate.source.is_empty() {
            return None;
        }
        Some(format!("{}/{}", candidate.source, skill_id))
    }

    fn flush_pending_project_global_detail_if_ready(&mut self) -> bool {
        detail_logic::flush_pending_project_global_detail_if_ready(
            &mut self.detail,
            self.search.search_client.as_ref(),
        )
    }

    fn active_skills_nodes(&self) -> &[SkillTreeNode] {
        match self.current_view {
            AppView::Project | AppView::Search => &self.project_skills_nodes,
            AppView::Global => &self.global_skills_nodes,
            AppView::Favorites => &self.favorites.nodes,
            AppView::Config => &self.project_skills_nodes,
        }
    }

    fn selected_skill_identity(&self) -> Option<(PathBuf, FavoriteSkill, String)> {
        let node = self.selected_skill_node()?;
        let skill_file = node.skill_file.as_ref()?.clone();
        let favorite = self.favorite_for_node(node)?;
        let remove_target = favorite
            .install_skill
            .rsplit('/')
            .next()
            .unwrap_or(&favorite.install_skill)
            .to_string();
        Some((skill_file, favorite, remove_target))
    }

    fn persist_favorites(&self) -> io::Result<()> {
        persist_user_config(&UserConfig {
            favorites: self.favorites.entries.clone(),
            ui: skills_tui::config::UiPreferences {
                show_markdown_pane: self.preview.show_markdown_pane,
                show_detail_pane: self.detail.show_detail_pane,
            },
        })
    }

    fn favorite_for_node(&self, node: &SkillTreeNode) -> Option<FavoriteSkill> {
        favorites_logic::favorite_for_node(node)
    }

    fn favorite_matches_dir_name(favorite: &FavoriteSkill, dir_name: &str) -> bool {
        if favorite.display_slug == dir_name || favorite.install_skill == dir_name {
            return true;
        }
        if let Some(source) = favorite.source.as_ref() {
            let full = format!("{}/{}", source, favorite.install_skill);
            if full == dir_name {
                return true;
            }
        }
        dir_name.ends_with(&format!("/{}", favorite.install_skill))
    }

    fn selected_favorite_entry(&self) -> Option<FavoriteSkill> {
        if self.current_view != AppView::Favorites {
            return None;
        }
        let node = self.selected_skill_node()?;
        self.favorites
            .entries
            .iter()
            .find(|favorite| Self::favorite_matches_dir_name(favorite, &node.dir_name))
            .cloned()
    }

    fn favorite_install_target(favorite: &FavoriteSkill) -> String {
        if let Some(source) = favorite.source.as_ref() {
            format!("{}/{}", source, favorite.install_skill)
        } else {
            favorite.install_skill.clone()
        }
    }

    fn skill_installed_in_nodes(nodes: &[SkillTreeNode], favorite: &FavoriteSkill) -> bool {
        for node in nodes {
            if let Some(skill_file) = node.skill_file.as_ref() {
                let slug = skill_remove_target_from_path(skill_file);
                if Self::favorite_matches_dir_name(favorite, &slug) {
                    return true;
                }
            }
            if Self::skill_installed_in_nodes(&node.children, favorite) {
                return true;
            }
        }
        false
    }

    fn install_selected_favorite(&mut self) {
        let Some(favorite) = self.selected_favorite_entry() else {
            self.show_toast("Select a favorite to install");
            return;
        };
        let install_target = Self::favorite_install_target(&favorite);
        self.begin_install_modal(
            install_target,
            "project (local)",
            false,
            InstallOrigin::Favorites,
        );
    }

    fn install_selected_favorite_global(&mut self) {
        let Some(favorite) = self.selected_favorite_entry() else {
            self.show_toast("Select a favorite to install");
            return;
        };
        let install_target = Self::favorite_install_target(&favorite);
        self.begin_install_modal(install_target, "global", true, InstallOrigin::Favorites);
    }

    fn toggle_favorite_slug(&mut self, favorite: FavoriteSkill) {
        if favorites_logic::toggle(&mut self.favorites.entries, favorite) {
            self.show_toast("Added to favorites");
        } else {
            self.show_toast("Removed from favorites");
        }

        if let Err(err) = self.persist_favorites() {
            self.show_toast(format!("Failed to save favorites: {}", err));
        }
        self.rebuild_favorites_nodes();
    }

    fn is_favorite_node(&self, node: &SkillTreeNode) -> bool {
        let Some(display_slug) = favorites_logic::display_slug_for_node(node) else {
            return false;
        };
        favorites_logic::contains_display_slug(&self.favorites.entries, &display_slug)
    }

    fn rebuild_favorites_nodes(&mut self) {
        self.favorites.nodes = favorites_logic::rebuild_nodes(
            &self.favorites.entries,
            &self.project_skills_nodes,
            &self.global_skills_nodes,
        );
    }

    fn refresh_skill_hierarchies(&mut self) {
        if let Ok(nodes) = load_project_skill_hierarchy() {
            self.project_skills_nodes = nodes;
        }
        if let Ok(nodes) = load_global_skill_hierarchy() {
            self.global_skills_nodes = nodes;
        }
        self.rebuild_favorites_nodes();
        self.ensure_skill_selection_visible();
        self.open_selected_file_immediate();
    }

    fn toggle_selected_favorite(&mut self) {
        if let Some(existing) = self.selected_favorite_entry() {
            self.toggle_favorite_slug(existing);
            return;
        }

        let Some(node) = self.selected_skill_node() else {
            self.show_toast("No selectable skill");
            return;
        };
        let Some(favorite) = self.favorite_for_node(node) else {
            self.show_toast("Cannot determine favorite metadata for this skill");
            return;
        };
        self.toggle_favorite_slug(favorite);
    }

    fn delete_selected_skill(&mut self) {
        let Some((_, favorite, remove_target)) = self.selected_skill_identity() else {
            self.show_toast("Select a skill file to delete");
            return;
        };

        let remove_result = if self.current_view == AppView::Global {
            remove_skill_noninteractive_scoped(
                &self.app_config.skills_command,
                &remove_target,
                true,
            )
        } else {
            remove_skill_noninteractive(&self.app_config.skills_command, &remove_target)
        };

        match remove_result {
            Ok(_) => {
                if let Ok(cwd) = std::env::current_dir() {
                    if let Err(err) = patch_project_lock_after_remove(&cwd, &favorite) {
                        self.show_toast(format!("Deleted skill, lock patch failed: {}", err));
                    }
                }
                self.refresh_skill_hierarchies();
                self.show_toast("Deleted selected skill");
            }
            Err(err) => {
                self.show_toast(format!("skills remove failed: {}", err));
            }
        }
    }

    fn update_selected_skill(&mut self) {
        if let Some(favorite) = self.selected_favorite_entry() {
            let slug = Self::favorite_install_target(&favorite);
            match run_configured_skills_command(&self.app_config.skills_command, &["add", &slug]) {
                Ok(_) => {
                    self.refresh_skill_hierarchies();
                    self.show_toast(format!("Installed {}", slug));
                }
                Err(err) => {
                    self.show_toast(format!("Install failed: {}", err));
                }
            }
            return;
        }

        let Some((_, _, skill_name)) = self.selected_skill_identity() else {
            self.show_toast("Select a skill file to update");
            return;
        };

        let _ =
            run_configured_skills_command(&self.app_config.skills_command, &["check", &skill_name]);
        match run_configured_skills_command(
            &self.app_config.skills_command,
            &["update", &skill_name],
        ) {
            Ok(_) => {
                self.refresh_skill_hierarchies();
                self.show_toast("Skill updated");
            }
            Err(err) => {
                self.show_toast(format!("Update failed: {}", err));
            }
        }
    }

    fn new(
        app_config: AppConfig,
        startup_dialog: Option<StartupDialogState>,
        source_path: PathBuf,
        source: SourceState,
        project_skills_nodes: Vec<SkillTreeNode>,
        global_skills_nodes: Vec<SkillTreeNode>,
    ) -> Self {
        let user_config = load_user_config().unwrap_or_else(|_| UserConfig::default());
        let show_toc = false;
        let mut preview = preview_logic::new_preview_state(source_path, source, show_toc);
        preview.show_markdown_pane = user_config.ui.show_markdown_pane;

        let mut skills_expanded = ExpandedSkillPaths::new();
        collect_expanded_skill_paths(&project_skills_nodes, &mut Vec::new(), &mut skills_expanded);

        let mut detail = DetailState::new();
        detail.show_detail_pane = user_config.ui.show_detail_pane;

        let mut app = Self {
            app_config,
            startup_dialog,
            preview,
            menu: Self::build_menu(),
            project_skills_nodes,
            global_skills_nodes,
            favorites: FavoritesState::new(user_config.favorites),
            skills_selected_path: None,
            skills_expanded,
            skills_offset: 0,
            current_view: AppView::Project,
            focus: FocusPane::Tree,
            navbar_area: Rect::default(),
            grid_area: Rect::default(),
            tree_area: Rect::default(),
            tree_content_area: Rect::default(),
            markdown_area: Rect::default(),
            detail_area: Rect::default(),
            markdown_inner_area: Rect::default(),
            last_move_processed: Instant::now(),
            toast_message: None,
            toast_expires_at: None,
            show_hotkeys_modal: false,
            delete_confirm_dialog: None,
            install_modal: None,
            agent_picker_modal: None,
            pending_startup_agent_picker: false,
            detail,
            search: SearchState::new(),
            config: ConfigState::new(),
        };

        app.pending_startup_agent_picker = app.app_config.skills_command.default_agents.is_empty();

        app.rebuild_favorites_nodes();
        app.reset_selection_to_first_skill();
        app.open_selected_file_immediate();
        if app.detail.show_detail_pane
            && matches!(
                app.current_view,
                AppView::Project | AppView::Global | AppView::Favorites
            )
        {
            let slug = app.selected_skill_identity().and_then(|(_, favorite, _)| {
                detail_logic::selected_project_global_slug(&favorite)
                    .or_else(|| app.resolve_global_slug_via_search(&favorite))
            });
            let _ = detail_logic::fetch_project_global_detail_now(
                &mut app.detail,
                app.search.search_client.as_ref(),
                slug.as_deref(),
            );
        }
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

    fn update_pane_areas_from_grid(&mut self) {
        self.tree_area = Rect::default();
        self.markdown_area = Rect::default();
        self.detail_area = Rect::default();

        if self.current_view == AppView::Search {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(self.grid_area);
            self.tree_area = cols[0];
            self.markdown_area = cols[1];
            return;
        }

        if self.current_view == AppView::Config {
            return;
        }

        let show_detail = self.detail.show_detail_pane
            && matches!(
                self.current_view,
                AppView::Project | AppView::Global | AppView::Favorites
            );
        let show_markdown = self.preview.show_markdown_pane;

        match (show_markdown, show_detail) {
            (true, true) => {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(20),
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                    ])
                    .split(self.grid_area);
                self.tree_area = cols[0];
                self.markdown_area = cols[1];
                self.detail_area = cols[2];
            }
            (true, false) => {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(self.grid_area);
                self.tree_area = cols[0];
                self.markdown_area = cols[1];
            }
            (false, true) => {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(self.grid_area);
                self.tree_area = cols[0];
                self.detail_area = cols[1];
            }
            (false, false) => {
                self.tree_area = self.grid_area;
            }
        }
    }

    fn queue_open_selected_file(&mut self) {
        let selected_path = self
            .selected_skill_node()
            .and_then(|node| node.skill_file.as_ref().cloned());
        preview_logic::queue_open_selected_file(&mut self.preview, selected_path);
    }

    fn open_selected_file_immediate(&mut self) {
        let selected_path = self
            .selected_skill_node()
            .and_then(|node| node.skill_file.as_ref().cloned());
        preview_logic::open_selected_file_immediate(&mut self.preview, selected_path);
    }

    fn selected_skill_node(&self) -> Option<&SkillTreeNode> {
        let path = self.skills_selected_path.as_ref()?;
        let mut nodes = self.active_skills_nodes();
        let mut current = None;
        for idx in path {
            let node = nodes.get(*idx)?;
            current = Some(node);
            nodes = &node.children;
        }
        current
    }

    fn visible_skill_paths(&self) -> Vec<SkillPath> {
        tree_nav_logic::visible_skill_paths(self.active_skills_nodes(), &self.skills_expanded)
    }

    fn ensure_skill_selection_visible(&mut self) {
        let active_nodes = match self.current_view {
            AppView::Project | AppView::Search | AppView::Config => &self.project_skills_nodes,
            AppView::Global => &self.global_skills_nodes,
            AppView::Favorites => &self.favorites.nodes,
        };
        tree_nav_logic::ensure_skill_selection_visible(
            active_nodes,
            &self.skills_expanded,
            &mut self.skills_selected_path,
            &mut self.skills_offset,
        );
    }

    fn select_next_skill(&mut self) {
        let active_nodes = match self.current_view {
            AppView::Project | AppView::Search | AppView::Config => &self.project_skills_nodes,
            AppView::Global => &self.global_skills_nodes,
            AppView::Favorites => &self.favorites.nodes,
        };
        tree_nav_logic::select_next_skill(
            active_nodes,
            &self.skills_expanded,
            &mut self.skills_selected_path,
        );
    }

    fn select_prev_skill(&mut self) {
        let active_nodes = match self.current_view {
            AppView::Project | AppView::Search | AppView::Config => &self.project_skills_nodes,
            AppView::Global => &self.global_skills_nodes,
            AppView::Favorites => &self.favorites.nodes,
        };
        tree_nav_logic::select_prev_skill(
            active_nodes,
            &self.skills_expanded,
            &mut self.skills_selected_path,
        );
    }

    fn expand_selected_skill(&mut self) {
        tree_nav_logic::expand_selected_skill(
            &self.skills_selected_path,
            &mut self.skills_expanded,
        );
    }

    fn collapse_selected_skill(&mut self) {
        tree_nav_logic::collapse_selected_skill(
            &mut self.skills_selected_path,
            &mut self.skills_expanded,
        );
    }

    fn flush_pending_preview_if_ready(&mut self) -> bool {
        preview_logic::flush_pending_preview_if_ready(&mut self.preview)
    }

    fn select_skill_by_visible_row(&mut self, row_index: usize) {
        let selected_before = self.skills_selected_path.clone();
        let active_nodes = match self.current_view {
            AppView::Project | AppView::Search | AppView::Config => &self.project_skills_nodes,
            AppView::Global => &self.global_skills_nodes,
            AppView::Favorites => &self.favorites.nodes,
        };
        tree_nav_logic::select_skill_by_visible_row(
            active_nodes,
            &self.skills_expanded,
            &mut self.skills_selected_path,
            &mut self.skills_offset,
            row_index,
        );
        if self.skills_selected_path != selected_before {
            self.queue_open_selected_file();
            if self.detail.show_detail_pane
                && matches!(
                    self.current_view,
                    AppView::Project | AppView::Global | AppView::Favorites
                )
            {
                self.queue_project_global_detail_refresh();
            }
        }
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

                if self.startup_dialog.is_some() {
                    if key.key_code == KeyCode::Char('c')
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                    {
                        return Ok(CoordinatorAction::Quit);
                    }
                    let mut toast_message: Option<String> = None;
                    if startup_dialog_logic::handle_startup_dialog_key(
                        &mut self.startup_dialog,
                        &mut self.app_config,
                        key.key_code,
                        key.modifiers,
                        |message| toast_message = Some(message),
                    ) {
                        if let Some(message) = toast_message {
                            self.show_toast(message);
                        }
                        return Ok(CoordinatorAction::Redraw);
                    }
                }

                if self.delete_confirm_dialog.is_some() {
                    if let Some(intent) = delete_confirm_logic::handle_delete_confirm_key(
                        &mut self.delete_confirm_dialog,
                        key.key_code,
                    ) {
                        if matches!(
                            intent,
                            delete_confirm_logic::DeleteConfirmIntent::ConfirmDelete
                        ) {
                            self.delete_selected_skill();
                        }
                        return Ok(CoordinatorAction::Redraw);
                    }
                }

                if let Some(modal) = self.install_modal.as_ref() {
                    if let InstallModalPhase::Finished { .. } = modal.phase {
                        if matches!(
                            key.key_code,
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('c')
                        ) {
                            self.install_modal = None;
                            return Ok(CoordinatorAction::Redraw);
                        }
                    }
                    return Ok(CoordinatorAction::Continue);
                }

                if let Some(modal) = self.agent_picker_modal.as_mut() {
                    let filtered = Self::filtered_agent_keys(&modal.query);
                    let save_row_index = filtered.len();
                    if modal.selected_index > save_row_index {
                        modal.selected_index = save_row_index;
                    }

                    match key.key_code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            modal.selected_index = modal.selected_index.saturating_sub(1);
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            modal.selected_index = (modal.selected_index + 1).min(save_row_index);
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Backspace => {
                            modal.query.pop();
                            modal.selected_index = 0;
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Esc => {
                            if !modal.required_on_startup {
                                self.agent_picker_modal = None;
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char(' ') => {
                            if modal.selected_index < filtered.len() {
                                let key_name = filtered[modal.selected_index].to_string();
                                if let Some(pos) = modal
                                    .selected_agents
                                    .iter()
                                    .position(|item| item == &key_name)
                                {
                                    modal.selected_agents.remove(pos);
                                } else {
                                    modal.selected_agents.push(key_name);
                                }
                                modal.selected_agents.sort();
                                modal.selected_agents.dedup();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Enter => {
                            self.save_agent_picker_selection();
                            self.agent_picker_modal = None;
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char(ch)
                            if !key
                                .modifiers
                                .intersects(crossterm::event::KeyModifiers::CONTROL)
                                && !key
                                    .modifiers
                                    .intersects(crossterm::event::KeyModifiers::ALT) =>
                        {
                            modal.query.push(ch);
                            modal.selected_index = 0;
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('s')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.save_agent_picker_selection();
                            self.agent_picker_modal = None;
                            return Ok(CoordinatorAction::Redraw);
                        }
                        _ => return Ok(CoordinatorAction::Continue),
                    }
                }

                let search_input_blocks_global =
                    self.current_view == AppView::Search && self.search.input_focused;

                if key.key_code == KeyCode::Char('q') && !search_input_blocks_global {
                    return Ok(CoordinatorAction::Quit);
                }

                if key.key_code == KeyCode::Esc
                    && self.show_hotkeys_modal
                    && !search_input_blocks_global
                {
                    self.show_hotkeys_modal = false;
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('?') && !search_input_blocks_global {
                    self.show_hotkeys_modal = !self.show_hotkeys_modal;
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.show_hotkeys_modal {
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Char('/') && self.current_view != AppView::Search {
                    self.set_view(AppView::Search);
                    self.search.input_focused = true;
                    self.search.search_status = "Search input focused".to_string();
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('1') && !search_input_blocks_global {
                    self.set_view(AppView::Project);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('2') && !search_input_blocks_global {
                    self.set_view(AppView::Global);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('3') && !search_input_blocks_global {
                    self.set_view(AppView::Search);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('4') && !search_input_blocks_global {
                    self.set_view(AppView::Favorites);
                    return Ok(CoordinatorAction::Redraw);
                }

                if key.key_code == KeyCode::Char('5') && !search_input_blocks_global {
                    self.set_view(AppView::Config);
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.current_view == AppView::Search {
                    if key.key_code == KeyCode::Tab {
                        self.focus = match self.focus {
                            FocusPane::Tree => FocusPane::Preview,
                            FocusPane::Preview => FocusPane::Tree,
                        };
                        return Ok(CoordinatorAction::Redraw);
                    }

                    match key.key_code {
                        KeyCode::Char('/') => {
                            self.search.input_focused = true;
                            self.search.search_status = "Search input focused".to_string();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Esc => {
                            self.search.input_focused = false;
                            self.search.search_status =
                                "Search input unfocused. Press / to focus input".to_string();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Backspace if self.search.input_focused => {
                            if !self.search.search_query.is_empty() {
                                self.search.search_query.pop();
                                self.queue_search_refresh();
                                return Ok(CoordinatorAction::Redraw);
                            }
                        }
                        KeyCode::Char('w')
                            if self.search.input_focused
                                && key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            let trimmed_len = self.search.search_query.trim_end().len();
                            self.search.search_query.truncate(trimmed_len);
                            if let Some(pos) = self.search.search_query.rfind(char::is_whitespace) {
                                self.search.search_query.truncate(pos + 1);
                            } else {
                                self.search.search_query.clear();
                            }
                            self.queue_search_refresh();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('u')
                            if self.search.input_focused
                                && key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.search.search_query.clear();
                            self.queue_search_refresh();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('r')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            self.refresh_search_results();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Up | KeyCode::Char('k') if !self.search.input_focused => {
                            if self.search.search_selected > 0 {
                                self.search.search_selected -= 1;
                                self.queue_selected_search_detail();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Down | KeyCode::Char('j') if !self.search.input_focused => {
                            if self.search.search_selected + 1 < self.search.search_results.len() {
                                self.search.search_selected += 1;
                                self.queue_selected_search_detail();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Enter if !self.search.input_focused => {
                            let slug = match self.selected_search_slug() {
                                Ok(slug) => slug,
                                Err(message) => {
                                    self.search.search_status = message;
                                    return Ok(CoordinatorAction::Redraw);
                                }
                            };
                            if self.is_search_skill_installed(&slug) {
                                if !self.open_installed_search_preview(&slug) {
                                    self.search.search_status =
                                        "Installed skill preview unavailable".to_string();
                                }
                            } else {
                                self.install_selected_search_skill();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('f') if !self.search.input_focused => {
                            self.toggle_selected_search_favorite();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('i') if !self.search.input_focused => {
                            self.install_selected_search_skill();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('u') if !self.search.input_focused => {
                            self.update_selected_search_skill();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char('r') | KeyCode::Delete if !self.search.input_focused => {
                            self.remove_selected_search_skill();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char(ch)
                            if self.search.input_focused
                                && !key
                                    .modifiers
                                    .intersects(crossterm::event::KeyModifiers::CONTROL)
                                && !key
                                    .modifiers
                                    .intersects(crossterm::event::KeyModifiers::ALT) =>
                        {
                            self.search.search_query.push(ch);
                            self.queue_search_refresh();
                            return Ok(CoordinatorAction::Redraw);
                        }
                        KeyCode::Char(ch)
                            if !key
                                .modifiers
                                .intersects(crossterm::event::KeyModifiers::CONTROL)
                                && !key
                                    .modifiers
                                    .intersects(crossterm::event::KeyModifiers::ALT) =>
                        {
                            let _ = ch;
                            return Ok(CoordinatorAction::Continue);
                        }
                        _ => {
                            return Ok(CoordinatorAction::Continue);
                        }
                    }
                }

                if self.current_view == AppView::Config {
                    if matches!(key.key_code, KeyCode::Enter) && self.config.selected_field == 2 {
                        self.open_agent_picker_modal(false);
                        return Ok(CoordinatorAction::Redraw);
                    }
                    let key_event = CrosstermKeyEvent {
                        code: key.key_code,
                        modifiers: key.modifiers,
                        kind: key.kind,
                        state: KeyEventState::NONE,
                    };
                    return Ok(config_logic::handle_config_key(
                        &mut self.config,
                        &mut self.app_config,
                        key_event,
                    ));
                }

                if key.key_code == KeyCode::Char(']') {
                    if matches!(
                        self.current_view,
                        AppView::Project | AppView::Global | AppView::Favorites
                    ) {
                        self.toggle_project_global_detail_pane();
                        return Ok(CoordinatorAction::Redraw);
                    }
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Char('[') {
                    if matches!(
                        self.current_view,
                        AppView::Project | AppView::Global | AppView::Favorites
                    ) {
                        self.toggle_project_global_markdown_pane();
                        return Ok(CoordinatorAction::Redraw);
                    }
                    return Ok(CoordinatorAction::Continue);
                }

                if key.key_code == KeyCode::Tab {
                    self.focus = match self.focus {
                        FocusPane::Tree => FocusPane::Preview,
                        FocusPane::Preview => FocusPane::Tree,
                    };
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.focus == FocusPane::Tree {
                    self.ensure_skill_selection_visible();
                    let selected_before = self.skills_selected_path.clone();
                    match key.key_code {
                        KeyCode::Delete | KeyCode::Char('d')
                            if self.current_view != AppView::Favorites =>
                        {
                            let Some((_, _, skill_name)) = self.selected_skill_identity() else {
                                self.show_toast("Select a skill file to delete");
                                return Ok(CoordinatorAction::Redraw);
                            };
                            self.delete_confirm_dialog =
                                Some(delete_confirm_logic::open_delete_confirm_dialog(skill_name));
                        }
                        KeyCode::Char('i') if self.current_view == AppView::Favorites => {
                            self.install_selected_favorite()
                        }
                        KeyCode::Char('I') if self.current_view == AppView::Favorites => {
                            self.install_selected_favorite_global()
                        }
                        KeyCode::Char('u') => self.update_selected_skill(),
                        KeyCode::Char('f') => self.toggle_selected_favorite(),
                        KeyCode::Down | KeyCode::Char('j') => self.select_next_skill(),
                        KeyCode::Up | KeyCode::Char('k') => self.select_prev_skill(),
                        KeyCode::Right | KeyCode::Char('l') => self.expand_selected_skill(),
                        KeyCode::Left | KeyCode::Char('h') => self.collapse_selected_skill(),
                        _ => {}
                    }
                    let selected_after = self.skills_selected_path.clone();
                    if selected_after != selected_before || key.key_code == KeyCode::Enter {
                        self.queue_open_selected_file();
                        if self.detail.show_detail_pane
                            && matches!(
                                self.current_view,
                                AppView::Project | AppView::Global | AppView::Favorites
                            )
                        {
                            self.refresh_project_global_detail_now();
                        }
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                let key_event = CrosstermKeyEvent {
                    code: key.key_code,
                    modifiers: key.modifiers,
                    kind: key.kind,
                    state: KeyEventState::NONE,
                };

                let markdown_event = self.preview.widget.handle_key(key_event);
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

                if self.startup_dialog.is_some() {
                    return Ok(CoordinatorAction::Continue);
                }

                if self.delete_confirm_dialog.is_some() {
                    return Ok(CoordinatorAction::Continue);
                }

                if self.install_modal.is_some() {
                    return Ok(CoordinatorAction::Continue);
                }

                if self.agent_picker_modal.is_some() {
                    return Ok(CoordinatorAction::Continue);
                }

                if Self::is_in_rect(self.navbar_area, mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::Moved => self.menu.update_hover(mouse.column, mouse.row),
                        MouseEventKind::Down(MouseButton::Left) => {
                            if let Some(index) = self.menu.handle_click(mouse.column, mouse.row) {
                                match index {
                                    0 => self.set_view(AppView::Project),
                                    1 => self.set_view(AppView::Global),
                                    2 => self.set_view(AppView::Search),
                                    3 => self.set_view(AppView::Favorites),
                                    4 => self.set_view(AppView::Config),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    return Ok(CoordinatorAction::Redraw);
                }

                if self.current_view == AppView::Search {
                    self.update_pane_areas_from_grid();
                    return Ok(CoordinatorAction::Continue);
                }

                if self.current_view == AppView::Config {
                    return Ok(CoordinatorAction::Continue);
                }

                self.update_pane_areas_from_grid();

                if Self::is_in_rect(self.tree_content_area, mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            self.focus = FocusPane::Tree;
                            let row_index =
                                mouse.row.saturating_sub(self.tree_content_area.y) as usize;
                            self.select_skill_by_visible_row(row_index);
                            self.queue_open_selected_file();
                            if self.detail.show_detail_pane
                                && matches!(
                                    self.current_view,
                                    AppView::Project | AppView::Global | AppView::Favorites
                                )
                            {
                                self.refresh_project_global_detail_now();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        MouseEventKind::ScrollDown => {
                            self.focus = FocusPane::Tree;
                            self.select_next_skill();
                            self.queue_open_selected_file();
                            if self.detail.show_detail_pane
                                && matches!(
                                    self.current_view,
                                    AppView::Project | AppView::Global | AppView::Favorites
                                )
                            {
                                self.refresh_project_global_detail_now();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        MouseEventKind::ScrollUp => {
                            self.focus = FocusPane::Tree;
                            self.select_prev_skill();
                            self.queue_open_selected_file();
                            if self.detail.show_detail_pane
                                && matches!(
                                    self.current_view,
                                    AppView::Project | AppView::Global | AppView::Favorites
                                )
                            {
                                self.refresh_project_global_detail_now();
                            }
                            return Ok(CoordinatorAction::Redraw);
                        }
                        _ => {}
                    }
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
                    .preview
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
                if self.pending_startup_agent_picker
                    && self.startup_dialog.is_none()
                    && self.agent_picker_modal.is_none()
                {
                    self.open_agent_picker_modal(true);
                    return Ok(CoordinatorAction::Redraw);
                }

                let toast_changed = self.clear_expired_toast();
                let preview_changed = self.flush_pending_preview_if_ready();
                let search_refresh_changed = self.flush_pending_search_refresh_if_ready();
                let search_detail_changed = self.flush_pending_search_detail_if_ready();
                let project_global_detail_changed =
                    self.flush_pending_project_global_detail_if_ready();
                let install_modal_changed = self.poll_install_modal_progress();
                if toast_changed
                    || preview_changed
                    || search_refresh_changed
                    || search_detail_changed
                    || project_global_detail_changed
                    || install_modal_changed
                {
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

        let footer_items = if self.current_view == AppView::Search {
            vec![
                HotkeyItem::new("hjkl", "Nav"),
                HotkeyItem::new("i", "install"),
                HotkeyItem::new("f", "favorite"),
                HotkeyItem::new("u", "update"),
                HotkeyItem::new("?", "hotkeys"),
                HotkeyItem::new("q", "quit"),
            ]
        } else {
            let mut items = vec![HotkeyItem::new("q", "quit")];
            if self.current_view == AppView::Project || self.current_view == AppView::Global {
                items.push(HotkeyItem::new("hjkl", "Nav"));
                items.push(HotkeyItem::new("d", "remove"));
                items.push(HotkeyItem::new("f", "favorite"));
                items.push(HotkeyItem::new("[ ]", "Toggle md/details"));
            } else if self.current_view == AppView::Favorites {
                items.push(HotkeyItem::new("hjkl", "Nav"));
                items.push(HotkeyItem::new("i", "install local"));
                items.push(HotkeyItem::new("I", "install global"));
                items.push(HotkeyItem::new("f", "favorite"));
                items.push(HotkeyItem::new("[ ]", "Toggle md/details"));
            }
            items.push(HotkeyItem::new("?", "hotkeys"));
            items
        };
        let footer = HotkeyFooter::new(footer_items);
        footer.render(frame, footer_area);

        if self.current_view == AppView::Search {
            self.update_pane_areas_from_grid();

            let selected_installed = self
                .selected_search_slug()
                .ok()
                .map(|slug| self.is_search_skill_installed(&slug))
                .unwrap_or(false);

            search_render::render_search_view(
                frame,
                self.tree_area,
                self.markdown_area,
                &mut self.search,
                &self.favorites.entries,
                &self.project_skills_nodes,
                &self.global_skills_nodes,
                self.focus == FocusPane::Tree,
                self.focus == FocusPane::Preview,
                TERMINAL_ICON,
                selected_installed,
            );
        } else if self.current_view == AppView::Config {
            let pane = Pane::new("Config (.agents/skills-tui-config.json)")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(UNFOCUSED_PANE_BORDER));
            let (inner, _) = pane.render_block(frame, self.grid_area);

            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(2)])
                .split(inner);
            let list_area = layout[0];
            let status_area = layout[1];

            let mut items = Vec::new();
            for i in 0..config_logic::config_field_count() {
                let label = if i == 1 {
                    config_logic::config_command_label(&self.app_config)
                } else {
                    config_logic::config_field_label(i)
                };
                let raw_value = config_logic::config_field_value(&self.app_config, i);
                let value = if i == self.config.selected_field {
                    config_logic::render_config_value_with_cursor(&self.config, &raw_value)
                } else {
                    raw_value
                };
                items.push(ListItem::new(format!("{:<28} {}", label, value)));
            }

            let list = List::new(items)
                .highlight_symbol("▶ ")
                .highlight_style(Style::default().fg(Color::Black).bg(YAZI_CYAN));
            let mut list_state = ListState::default();
            list_state.select(Some(self.config.selected_field));
            frame.render_stateful_widget(list, list_area, &mut list_state);

            let dirty_mark = if self.config.dirty { "*" } else { "" };
            let status = format!("{}{}", self.config.status, dirty_mark);
            let status_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(1)])
                .split(status_area);
            frame.render_widget(
                Paragraph::new(status).style(Style::default().fg(Color::DarkGray)),
                status_rows[0],
            );
            let config_hotkeys = HotkeyFooter::new(vec![
                HotkeyItem::new("j/k", "field"),
                HotkeyItem::new("h/l", "cursor/toggle"),
                HotkeyItem::new("type", "edit"),
                HotkeyItem::new("ctrl+s", "save"),
            ]);
            config_hotkeys.render(frame, status_rows[1]);
        } else {
            self.update_pane_areas_from_grid();
            self.markdown_inner_area = Rect::default();

            if self.tree_area.width > 0 && self.tree_area.height > 0 {
                let tree_border = if self.focus == FocusPane::Tree {
                    Color::Blue
                } else {
                    UNFOCUSED_PANE_BORDER
                };
                let tree_title = if self.current_view == AppView::Global {
                    "Global Skills"
                } else if self.current_view == AppView::Favorites {
                    "Favorite Skills"
                } else {
                    "Skills"
                };
                let tree_pane = Pane::new(tree_title)
                    .with_icon(TERMINAL_ICON)
                    .border_style(Style::default().fg(tree_border));
                let (tree_inner, _) = tree_pane.render_block(frame, self.tree_area);
                let tree_rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0)])
                    .split(tree_inner);

                let mut tree_list_area = tree_rows[0];
                if self.current_view == AppView::Favorites {
                    let sections = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(1), Constraint::Min(0)])
                        .split(tree_rows[0]);
                    let mut header = format!("{:<30} {:<7} {:<6}", "Name", "Project", "Global");
                    if sections[0].width > 0 {
                        header = truncate_to_width(&header, sections[0].width as usize);
                    }
                    frame.render_widget(
                        Paragraph::new(header).style(Style::default().fg(Color::DarkGray)),
                        sections[0],
                    );
                    tree_list_area = sections[1];
                }
                self.tree_content_area = tree_list_area;

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
                    if let Some(node) = skill_node_at_path(self.active_skills_nodes(), path) {
                        let depth = path.len().saturating_sub(1);
                        let indent = "  ".repeat(depth);
                        let is_selected = self
                            .skills_selected_path
                            .as_ref()
                            .map(|p| p == path)
                            .unwrap_or(false);
                        let is_favorite =
                            self.current_view != AppView::Favorites && self.is_favorite_node(node);
                        let _is_expanded = self.skills_expanded.contains(path);
                        let row_style = if is_selected && is_favorite {
                            Style::default().fg(Color::Black).bg(FAVORITE_ORANGE)
                        } else if is_selected {
                            Style::default().fg(Color::Black).bg(YAZI_CYAN)
                        } else if is_favorite {
                            Style::default().fg(FAVORITE_ORANGE)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        let favorite_marker = if self.current_view == AppView::Favorites {
                            " "
                        } else if is_favorite {
                            "★"
                        } else {
                            " "
                        };
                        if self.current_view == AppView::Favorites {
                            let project_installed = self
                                .favorites
                                .entries
                                .iter()
                                .find(|favorite| {
                                    Self::favorite_matches_dir_name(favorite, &node.dir_name)
                                })
                                .map(|favorite| {
                                    Self::skill_installed_in_nodes(
                                        &self.project_skills_nodes,
                                        favorite,
                                    )
                                })
                                .unwrap_or(false);
                            let global_installed = self
                                .favorites
                                .entries
                                .iter()
                                .find(|favorite| {
                                    Self::favorite_matches_dir_name(favorite, &node.dir_name)
                                })
                                .map(|favorite| {
                                    Self::skill_installed_in_nodes(
                                        &self.global_skills_nodes,
                                        favorite,
                                    )
                                })
                                .unwrap_or(false);
                            let name = truncate_to_width(&node.display_name, 30);

                            let base_fg = if is_selected {
                                Color::Black
                            } else {
                                Color::White
                            };
                            let base_style = if is_selected {
                                Style::default().fg(base_fg).bg(YAZI_CYAN)
                            } else {
                                Style::default().fg(base_fg)
                            };
                            let project_style = if is_selected {
                                Style::default()
                                    .fg(if project_installed {
                                        FAVORITE_ORANGE
                                    } else {
                                        base_fg
                                    })
                                    .bg(YAZI_CYAN)
                            } else {
                                Style::default().fg(if project_installed {
                                    FAVORITE_ORANGE
                                } else {
                                    base_fg
                                })
                            };
                            let global_style = if is_selected {
                                Style::default()
                                    .fg(if global_installed {
                                        FAVORITE_ORANGE
                                    } else {
                                        base_fg
                                    })
                                    .bg(YAZI_CYAN)
                            } else {
                                Style::default().fg(if global_installed {
                                    FAVORITE_ORANGE
                                } else {
                                    base_fg
                                })
                            };

                            let mut spans = vec![
                                Span::styled(format!("{:<30}", name), base_style),
                                Span::styled(" ", base_style),
                                Span::styled(
                                    format!("{:<7}", if project_installed { "●" } else { "○" }),
                                    project_style,
                                ),
                                Span::styled(" ", base_style),
                                Span::styled(
                                    format!("{:<6}", if global_installed { "●" } else { "○" }),
                                    global_style,
                                ),
                            ];

                            let fixed_len = 30 + 1 + 7 + 1 + 6;
                            if line_width > fixed_len {
                                spans.push(Span::styled(
                                    " ".repeat(line_width - fixed_len),
                                    base_style,
                                ));
                            }
                            lines.push(Line::from(spans));
                        } else {
                            let text = format!(
                                "{}{} {} {}",
                                indent, favorite_marker, TERMINAL_ICON, node.display_name
                            );
                            let padded = if line_width > 0 {
                                format!("{:<width$}", text, width = line_width)
                            } else {
                                text
                            };
                            lines.push(Line::from(Span::styled(padded, row_style)));
                        }
                    }
                }

                frame.render_widget(Paragraph::new(lines), self.tree_content_area);
            }

            if self.preview.show_markdown_pane
                && self.markdown_area.width > 0
                && self.markdown_area.height > 0
            {
                let preview_border = if self.focus == FocusPane::Preview {
                    Color::Blue
                } else {
                    UNFOCUSED_PANE_BORDER
                };
                let preview_title = if self.focus == FocusPane::Preview {
                    preview_relative_to_skills(&self.preview.source_path)
                } else {
                    self.preview.preview_title.clone()
                };
                let preview_pane = Pane::new(preview_title)
                    .with_icon(TERMINAL_ICON)
                    .border_style(Style::default().fg(preview_border));
                let (preview_inner, _) = preview_pane.render_block(frame, self.markdown_area);
                self.markdown_inner_area = preview_inner;
                if preview_inner.width > 0 && preview_inner.height > 0 {
                    frame.render_widget(&mut self.preview.widget, preview_inner);
                }
            }

            if self.detail.show_detail_pane
                && self.detail_area.width > 0
                && self.detail_area.height > 0
                && matches!(
                    self.current_view,
                    AppView::Project | AppView::Global | AppView::Favorites
                )
            {
                render_skill_detail_pane(
                    frame,
                    self.detail_area,
                    TERMINAL_ICON,
                    UNFOCUSED_PANE_BORDER,
                    SkillDetailPaneData {
                        detail: self.detail.project_global_detail.as_ref(),
                        empty_line_1: "No detail for selected skill.",
                        empty_line_2: "Select a skill under Project/Global.",
                    },
                );
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
                Line::from(" [           toggle markdown pane (Project/Global/Favorites)"),
                Line::from(" ]           toggle detail pane (Project/Global/Favorites)"),
                Line::from(" ? / esc     open/close hotkeys"),
                Line::from(" 1 / 2 / 3 / 4 / 5 switch Project/Global/Search/Favorites/Config"),
                Line::from(" d/delete    remove skill"),
                Line::from(" u           update skill"),
                Line::from(" f           toggle favorite"),
                Line::from(" tab         switch tree/preview focus"),
                Line::from(""),
                Line::from("File Tree"),
                Line::from(" up/down     move selection"),
                Line::from(" enter       expand/collapse or open file"),
                Line::from(" /           filter tree"),
                Line::from(""),
                Line::from("Markdown / Config"),
                Line::from(" mouse wheel scroll"),
                Line::from(" drag/select copy (shows toast)"),
                Line::from(" Ctrl+S save config (Config tab)"),
            ];
            hotkeys_pane.render_paragraph(frame, modal_area, hotkeys);
        }

        if let Some(state) = self.startup_dialog.as_mut() {
            let focused_pane_border = match self.focus {
                FocusPane::Tree => Color::Blue,
                FocusPane::Preview => Color::Blue,
            };
            match state {
                StartupDialogState::Info { title, message } => {
                    let mut dialog = Dialog::success(title, message)
                        .buttons(vec!["OK"])
                        .width_percent(0.62)
                        .height_percent(0.38)
                        .border_color(focused_pane_border)
                        .overlay(true)
                        .content_padding(2, 1)
                        .message_alignment(Alignment::Left)
                        .wrap_mode(DialogWrap::WordTrim)
                        .shadow(DialogShadow::Medium)
                        .hide_footer();
                    frame.render_widget(DialogWidget::new(&mut dialog), root_area);
                }
                StartupDialogState::ChooseCommand {
                    selected_button,
                    error_message,
                } => {
                    let mut message = startup_dialog_logic::startup_choice_message().to_string();
                    if let Some(error_message) = error_message {
                        message.push_str("\n\n");
                        message.push_str(error_message);
                    }

                    let mut dialog = Dialog::warning("Configuration", &message)
                        .buttons(vec!["Install Global Skills", "Use npx skills"])
                        .default_selection(*selected_button)
                        .actions_layout(DialogActionsLayout::Vertical)
                        .actions_alignment(Alignment::Left)
                        .width_percent(0.58)
                        .height_percent(0.34)
                        .border_color(focused_pane_border)
                        .overlay(true)
                        .content_padding(2, 1)
                        .message_alignment(Alignment::Left)
                        .wrap_mode(DialogWrap::WordTrim)
                        .shadow(DialogShadow::Medium)
                        .hide_footer();
                    frame.render_widget(DialogWidget::new(&mut dialog), root_area);
                    *selected_button = dialog.selected_button;
                }
            }
        }

        if let Some(state) = self.delete_confirm_dialog.as_mut() {
            let message = format!(
                "Delete skill '{}' ?\n\nThis action cannot be undone.",
                state.skill_name
            );
            let mut dialog = Dialog::confirm("Configuration", &message)
                .buttons(vec!["Yes", "No"])
                .default_selection(state.selected_button)
                .width_percent(0.50)
                .height_percent(0.28)
                .overlay(true)
                .content_padding(2, 1)
                .message_alignment(Alignment::Left)
                .wrap_mode(DialogWrap::WordTrim)
                .shadow(DialogShadow::Medium)
                .hide_footer();
            frame.render_widget(DialogWidget::new(&mut dialog), root_area);
            state.selected_button = dialog.selected_button;
        }

        if let Some(state) = self.install_modal.as_mut() {
            match &state.phase {
                InstallModalPhase::Installing => {
                    let message = format!(
                        "Installing skill...\n\nSlug: {}\nScope: {}\n\nPlease wait.",
                        state.slug, state.scope_label
                    );
                    let mut dialog = Dialog::info("Installing Skill", &message)
                        .width_percent(0.58)
                        .height_percent(0.30)
                        .overlay(true)
                        .content_padding(2, 1)
                        .message_alignment(Alignment::Left)
                        .wrap_mode(DialogWrap::WordTrim)
                        .shadow(DialogShadow::Medium)
                        .hide_footer();
                    frame.render_widget(DialogWidget::new(&mut dialog), root_area);
                }
                InstallModalPhase::Finished { success, message } => {
                    let title = if *success {
                        "Install Complete"
                    } else {
                        "Install Failed"
                    };
                    let mut dialog = if *success {
                        Dialog::success(title, message)
                    } else {
                        Dialog::warning(title, message)
                    }
                    .buttons(vec!["Continue"])
                    .default_selection(0)
                    .width_percent(0.58)
                    .height_percent(0.30)
                    .overlay(true)
                    .content_padding(2, 1)
                    .message_alignment(Alignment::Left)
                    .wrap_mode(DialogWrap::WordTrim)
                    .shadow(DialogShadow::Medium);
                    frame.render_widget(DialogWidget::new(&mut dialog), root_area);
                }
            }
        }

        if let Some(state) = self.agent_picker_modal.as_mut() {
            let modal_width = root_area.width.min(86);
            let modal_height = root_area.height.min(26);
            let modal_area = Rect {
                x: root_area.x + (root_area.width.saturating_sub(modal_width)) / 2,
                y: root_area.y + (root_area.height.saturating_sub(modal_height)) / 2,
                width: modal_width,
                height: modal_height,
            };

            frame.render_widget(Clear, modal_area);
            let pane = Pane::new("Default Agents")
                .with_icon(TERMINAL_ICON)
                .border_style(Style::default().fg(Color::Green));
            let (inner, _) = pane.render_block(frame, modal_area);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(2),
                ])
                .split(inner);

            frame.render_widget(
                Paragraph::new("Select default install agents (multi-select)")
                    .style(Style::default().fg(Color::White)),
                rows[0],
            );
            frame.render_widget(
                Paragraph::new(format!("Search: {}", state.query))
                    .style(Style::default().fg(Color::DarkGray)),
                rows[1],
            );

            let filtered = Self::filtered_agent_keys(&state.query);
            let list_height = rows[2].height as usize;
            let mut lines = Vec::new();
            for (idx, agent) in filtered
                .iter()
                .take(list_height.saturating_sub(1))
                .enumerate()
            {
                let selected = state.selected_agents.iter().any(|item| item == agent);
                let focused = idx == state.selected_index;
                let marker = if selected { "●" } else { "○" };
                let style = if focused {
                    Style::default().fg(Color::Black).bg(YAZI_CYAN)
                } else if selected {
                    Style::default().fg(FAVORITE_ORANGE)
                } else {
                    Style::default().fg(Color::White)
                };
                lines.push(Line::from(Span::styled(
                    format!("{} {}", marker, agent),
                    style,
                )));
            }

            let save_row_index = filtered.len();
            let save_focused = state.selected_index == save_row_index;
            let save_style = if save_focused {
                Style::default().fg(Color::Black).bg(YAZI_CYAN)
            } else {
                Style::default().fg(Color::Green)
            };
            lines.push(Line::from(Span::styled(
                "[ Save default agents ]",
                save_style,
            )));

            frame.render_widget(Paragraph::new(lines), rows[2]);

            let footer = if state.required_on_startup {
                "Enter/Space toggle, Enter on Save to continue"
            } else {
                "Enter/Space toggle, Enter on Save to persist, Esc close"
            };
            frame.render_widget(
                Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
                rows[3],
            );
        }
    }
}

fn initialize_startup_state() -> io::Result<(AppConfig, Option<StartupDialogState>)> {
    let startup = initialize_app_config()?;

    if startup.existing_config {
        return Ok((startup.config, None));
    }

    let startup_dialog = if startup.config.skills_command.global_command_verified {
        Some(StartupDialogState::Info {
            title: "Configuration".to_string(),
            message: format!(
                "Created {} and verified global '{}' ({})",
                APP_CONFIG_PATH,
                startup.config.skills_command.global_command,
                startup
                    .verified_version
                    .unwrap_or_else(|| "version unknown".to_string())
            ),
        })
    } else {
        Some(StartupDialogState::ChooseCommand {
            selected_button: 0,
            error_message: None,
        })
    };

    Ok((startup.config, startup_dialog))
}

fn main() -> io::Result<()> {
    let (app_config, startup_dialog) = initialize_startup_state()?;

    let project_skills_nodes = load_project_skill_hierarchy()?;
    let global_skills_nodes = load_global_skill_hierarchy()?;

    let source_path = first_skill_file(&project_skills_nodes)
        .or_else(|| first_skill_file(&global_skills_nodes))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SKILL_PATH));
    if !source_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No SKILL.md found in project (.agents) or supported global skill directories",
        ));
    }
    let source = load_source_from_path(&source_path)?;
    let mut app = SkillPreviewApp::new(
        app_config,
        startup_dialog,
        source_path,
        source,
        project_skills_nodes,
        global_skills_nodes,
    );
    if app.project_skills_nodes.is_empty() && !app.global_skills_nodes.is_empty() {
        app.set_view(AppView::Global);
    }
    let config = RunnerConfig {
        tick_rate: Duration::from_millis(250),
        ..RunnerConfig::default()
    };
    run(app, config)
}
