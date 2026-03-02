pub struct ConfigState {
    pub selected_field: usize,
    pub value_cursor: usize,
    pub status: String,
    pub dirty: bool,
}

impl ConfigState {
    pub fn new() -> Self {
        Self {
            selected_field: 0,
            value_cursor: 0,
            status: "Edit values. Ctrl+S to save.".to_string(),
            dirty: false,
        }
    }
}
