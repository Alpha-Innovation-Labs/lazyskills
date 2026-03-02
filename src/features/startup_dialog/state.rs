pub enum StartupDialogState {
    Info {
        title: String,
        message: String,
    },
    ChooseCommand {
        selected_button: usize,
        error_message: Option<String>,
    },
}
