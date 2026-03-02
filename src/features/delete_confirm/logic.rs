use crossterm::event::KeyCode;

use super::state::DeleteConfirmDialogState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteConfirmIntent {
    None,
    Close,
    ConfirmDelete,
}

pub fn open_delete_confirm_dialog(skill_name: String) -> DeleteConfirmDialogState {
    DeleteConfirmDialogState {
        selected_button: 0,
        skill_name,
    }
}

pub fn handle_delete_confirm_key(
    state: &mut Option<DeleteConfirmDialogState>,
    key_code: KeyCode,
) -> Option<DeleteConfirmIntent> {
    let Some(state_value) = state.as_mut() else {
        return None;
    };

    let intent = match key_code {
        KeyCode::Left | KeyCode::Char('h') => {
            state_value.selected_button = state_value.selected_button.saturating_sub(1);
            DeleteConfirmIntent::None
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            state_value.selected_button = (state_value.selected_button + 1) % 2;
            DeleteConfirmIntent::None
        }
        KeyCode::Esc => {
            *state = None;
            DeleteConfirmIntent::Close
        }
        KeyCode::Char('d') => {
            *state = None;
            DeleteConfirmIntent::ConfirmDelete
        }
        KeyCode::Enter => {
            let choice = state_value.selected_button;
            *state = None;
            if choice == 0 {
                DeleteConfirmIntent::ConfirmDelete
            } else {
                DeleteConfirmIntent::Close
            }
        }
        _ => DeleteConfirmIntent::None,
    };

    Some(intent)
}
