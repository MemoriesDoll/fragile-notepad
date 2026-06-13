use iced::Task;

use super::App;
use crate::message::Message;

impl App {
    pub(super) fn update_menu(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MenuToggled(menu) => {
                if self.active_menu == Some(menu) {
                    self.active_menu = None;
                    self.active_menu_path.clear();
                } else {
                    self.active_menu = Some(menu);
                    self.active_menu_path.clear();
                }
            }
            Message::MenuHovered(menu) => {
                if self.active_menu.is_some() {
                    self.active_menu = Some(menu);
                    self.active_menu_path.clear();
                }
            }
            Message::MenuPathHovered(path) => {
                if self.active_menu.is_some() {
                    self.active_menu_path = path.segments;
                }
            }
            Message::MenuClosed => {
                self.active_menu = None;
                self.active_menu_path.clear();
            }
            _ => unreachable!("menu handler received non-menu message"),
        }

        Task::none()
    }
}
