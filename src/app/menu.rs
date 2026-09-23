//! Menu interaction owns its active menu and submenu path together.

use crate::message::{Menu, MenuMessage};

#[derive(Debug, Default)]
pub(super) struct MenuState {
    active: Option<Menu>,
    path: Vec<String>,
}

impl MenuState {
    pub(super) fn active(&self) -> Option<Menu> {
        self.active
    }
    pub(super) fn path(&self) -> &[String] {
        &self.path
    }
    pub(super) fn close(&mut self) {
        self.active = None;
        self.path.clear();
    }
    pub(super) fn update(&mut self, message: MenuMessage) {
        match message {
            MenuMessage::MenuToggled(menu) => {
                self.active = if self.active == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                self.path.clear();
            }
            MenuMessage::MenuHovered(menu) => {
                if self.active.is_some() {
                    self.active = Some(menu);
                    self.path.clear();
                }
            }
            MenuMessage::MenuPathHovered(path) => {
                if self.active.is_some() {
                    self.path = path.segments;
                }
            }
            MenuMessage::MenuClosed => self.close(),
        }
    }
}
