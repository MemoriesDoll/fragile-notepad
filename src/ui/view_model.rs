//! Inputs required to render the workbench; independent of application state ownership.

use super::{
    ChromeAnimationInfo, about_dialog, toolbar::WindowMenuState,
    window_list_dialog::WindowListEntry,
};
use crate::core::{Document, DocumentId, EditorSettings, FindState, Workspace};
use crate::editor::OutlineState;
use crate::message::{AboutTab, Menu};

pub struct WorkbenchView<'a> {
    pub workspace: &'a Workspace,
    pub find: &'a FindState,
    pub settings: &'a EditorSettings,
    pub is_find_visible: bool,
    pub is_inline_replace_visible: bool,
    pub is_function_list_visible: bool,
    pub chrome_animation: ChromeAnimationInfo,
    pub active_menu: Option<Menu>,
    pub active_menu_path: &'a [String],
    pub window_menu_state: WindowMenuState,
    pub dragged_tab: Option<DocumentId>,
    pub hovered_drop_tab: Option<DocumentId>,
    pub dirty_close_document: Option<&'a Document>,
    pub about_tab: Option<AboutTab>,
    pub rendering_debug_info: about_dialog::RenderingDebugInfo,
    pub window_list_entries: Option<Vec<WindowListEntry>>,
    pub file_status: Option<&'a str>,
    pub active_outline_state: Option<&'a OutlineState>,
}
