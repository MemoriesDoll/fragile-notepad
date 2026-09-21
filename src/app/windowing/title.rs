use crate::core::Workspace;

pub(crate) trait Title {
    fn title(&self) -> String;
}

impl Title for Workspace {
    fn title(&self) -> String {
        self.active_document()
            .map(|document| format!("{} - Fragile Notepad", document.title()))
            .unwrap_or_else(|| String::from("Fragile Notepad"))
    }
}
