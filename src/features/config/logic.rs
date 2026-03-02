use crossterm::event::{KeyCode, KeyEvent as CrosstermKeyEvent, KeyModifiers};
use ratkit::prelude::CoordinatorAction;
use skills_tui::config::{persist_app_config, AppConfig, SkillsCommandMode, APP_CONFIG_PATH};

use super::state::ConfigState;

pub fn config_field_count() -> usize {
    3
}

pub fn config_field_label(index: usize) -> &'static str {
    match index {
        0 => "mode",
        1 => "command",
        2 => "default_agents",
        _ => "",
    }
}

pub fn config_command_label(app_config: &AppConfig) -> &'static str {
    match app_config.skills_command.mode {
        SkillsCommandMode::Global => "global_command",
        SkillsCommandMode::Npx => "npx_command",
    }
}

pub fn config_field_value(app_config: &AppConfig, index: usize) -> String {
    match index {
        0 => match app_config.skills_command.mode {
            SkillsCommandMode::Global => "global".to_string(),
            SkillsCommandMode::Npx => "npx".to_string(),
        },
        1 => match app_config.skills_command.mode {
            SkillsCommandMode::Global => app_config.skills_command.global_command.clone(),
            SkillsCommandMode::Npx => app_config.skills_command.npx_command.clone(),
        },
        2 => {
            if app_config.skills_command.default_agents.is_empty() {
                "<not set> (press Enter to edit)".to_string()
            } else {
                app_config.skills_command.default_agents.join(", ")
            }
        }
        _ => String::new(),
    }
}

fn set_selected_config_value(state: &ConfigState, app_config: &mut AppConfig, value: String) {
    if state.selected_field == 1 {
        match app_config.skills_command.mode {
            SkillsCommandMode::Global => app_config.skills_command.global_command = value,
            SkillsCommandMode::Npx => app_config.skills_command.npx_command = value,
        }
    }
}

fn is_text_config_field(state: &ConfigState) -> bool {
    state.selected_field == 1
}

fn toggle_config_field(state: &mut ConfigState, app_config: &mut AppConfig) {
    if state.selected_field == 0 {
        app_config.skills_command.mode = match app_config.skills_command.mode {
            SkillsCommandMode::Global => SkillsCommandMode::Npx,
            SkillsCommandMode::Npx => SkillsCommandMode::Global,
        };
        state.dirty = true;
    }
}

pub fn render_config_value_with_cursor(state: &ConfigState, value: &str) -> String {
    if !is_text_config_field(state) {
        return value.to_string();
    }

    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    for (idx, ch) in chars.iter().enumerate() {
        if idx == state.value_cursor {
            out.push('▏');
        }
        out.push(*ch);
    }
    if state.value_cursor >= chars.len() {
        out.push('▏');
    }
    out
}

pub fn handle_config_key(
    state: &mut ConfigState,
    app_config: &mut AppConfig,
    key: CrosstermKeyEvent,
) -> CoordinatorAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('s')) {
        match persist_app_config(app_config) {
            Ok(_) => {
                state.dirty = false;
                state.status = format!("Saved {}", APP_CONFIG_PATH);
            }
            Err(err) => {
                state.status = format!("Save failed: {err}");
            }
        }
        return CoordinatorAction::Redraw;
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.selected_field = state.selected_field.saturating_sub(1);
            state.value_cursor = config_field_value(app_config, state.selected_field)
                .chars()
                .count();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.selected_field =
                (state.selected_field + 1).min(config_field_count().saturating_sub(1));
            state.value_cursor = config_field_value(app_config, state.selected_field)
                .chars()
                .count();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if is_text_config_field(state) {
                state.value_cursor = state.value_cursor.saturating_sub(1);
            } else {
                toggle_config_field(state, app_config);
            }
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            if is_text_config_field(state) {
                let len = config_field_value(app_config, state.selected_field)
                    .chars()
                    .count();
                state.value_cursor = (state.value_cursor + 1).min(len);
            } else {
                toggle_config_field(state, app_config);
            }
        }
        KeyCode::Backspace if is_text_config_field(state) => {
            let mut value = config_field_value(app_config, state.selected_field);
            if state.value_cursor > 0 {
                let mut chars: Vec<char> = value.chars().collect();
                let idx = state.value_cursor - 1;
                chars.remove(idx);
                value = chars.into_iter().collect();
                state.value_cursor = idx;
                set_selected_config_value(state, app_config, value);
                state.dirty = true;
            }
        }
        KeyCode::Delete if is_text_config_field(state) => {
            let mut value = config_field_value(app_config, state.selected_field);
            let mut chars: Vec<char> = value.chars().collect();
            if state.value_cursor < chars.len() {
                chars.remove(state.value_cursor);
                value = chars.into_iter().collect();
                set_selected_config_value(state, app_config, value);
                state.dirty = true;
            }
        }
        KeyCode::Char(ch)
            if is_text_config_field(state)
                && !key.modifiers.intersects(KeyModifiers::CONTROL)
                && !key.modifiers.intersects(KeyModifiers::ALT) =>
        {
            let mut value = config_field_value(app_config, state.selected_field);
            let mut chars: Vec<char> = value.chars().collect();
            chars.insert(state.value_cursor, ch);
            value = chars.into_iter().collect();
            state.value_cursor += 1;
            set_selected_config_value(state, app_config, value);
            state.dirty = true;
        }
        _ => {}
    }

    CoordinatorAction::Redraw
}
