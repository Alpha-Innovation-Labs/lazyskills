use crossterm::event::{KeyCode, KeyModifiers};
use lazyskills::{
    config::{persist_app_config, AppConfig, SkillsCommandMode},
    services::skills_command::{install_global_skills_cli, verify_global_skills_command},
};

use super::state::StartupDialogState;

pub fn startup_choice_message() -> &'static str {
    "Global 'skills' could not be verified as the expected skills CLI.\n\nChoose how this project should run skills commands."
}

pub fn apply_startup_choice(
    app_config: &mut AppConfig,
    startup_dialog: &mut Option<StartupDialogState>,
    selected_button: usize,
    show_toast: &mut dyn FnMut(String),
) {
    if selected_button == 0 {
        if let Err(message) = install_global_skills_cli() {
            *startup_dialog = Some(StartupDialogState::ChooseCommand {
                selected_button,
                error_message: Some(message),
            });
            return;
        }

        app_config.skills_command.mode = SkillsCommandMode::Global;
        let verified_version = verify_global_skills_command(&app_config.skills_command);
        app_config.skills_command.global_command_verified = verified_version.is_some();
        app_config.skills_command.global_command_version = verified_version;
    } else {
        app_config.skills_command.mode = SkillsCommandMode::Npx;
        app_config.skills_command.global_command_verified = false;
        app_config.skills_command.global_command_version = None;
    }

    if let Err(err) = persist_app_config(app_config) {
        show_toast(format!("Failed to save config: {}", err));
        return;
    }

    *startup_dialog = None;
}

pub fn handle_startup_dialog_key(
    startup_dialog: &mut Option<StartupDialogState>,
    app_config: &mut AppConfig,
    key_code: KeyCode,
    _modifiers: KeyModifiers,
    mut show_toast: impl FnMut(String),
) -> bool {
    let mut close_dialog = false;
    let mut pending_choice: Option<usize> = None;

    if let Some(state) = startup_dialog.as_mut() {
        match state {
            StartupDialogState::Info { .. } => {
                if matches!(key_code, KeyCode::Enter | KeyCode::Esc) {
                    close_dialog = true;
                }
            }
            StartupDialogState::ChooseCommand {
                selected_button,
                error_message,
            } => match key_code {
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('1') => {
                    *selected_button = selected_button.saturating_sub(1);
                    *error_message = None;
                }
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab | KeyCode::Char('2') => {
                    *selected_button = (*selected_button + 1).min(1);
                    *error_message = None;
                }
                KeyCode::Enter => pending_choice = Some(*selected_button),
                _ => {}
            },
        }
    }

    if close_dialog {
        *startup_dialog = None;
    }

    if let Some(choice) = pending_choice {
        apply_startup_choice(app_config, startup_dialog, choice, &mut show_toast);
    }

    true
}
