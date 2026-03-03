pub enum StartupDialogState {
    ChooseCommand {
        selected_button: usize,
        error_message: Option<String>,
    },
}
