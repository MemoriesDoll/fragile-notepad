use iced::highlighter;
use iced::widget::{
    button, column, container, image, mouse_area, opaque, row, scrollable, space, stack, text,
    tooltip,
};
use iced::{Center, Element, Fill, Length};

use crate::core::{EditorSettings, ShortcutCommand, TextEncoding};
use crate::message::{Menu, Message};
use crate::ui::icons::hero::{self, HeroIcon, IconTone};
use crate::ui::icons::tango::{self, TangoIcon};
use crate::ui::styles;
use crate::ui::{
    centered_button_content,
    menu::{self, MenuNode, MenuTree},
};

const MENU_BAR_PADDING: [u16; 2] = [1, 4];
const MENU_LABEL_PADDING: [u16; 2] = [2, 8];
const MENU_ITEMS: &[(Menu, &str)] = &[
    (Menu::File, "File"),
    (Menu::Edit, "Edit"),
    (Menu::Search, "Search"),
    (Menu::View, "View"),
    (Menu::Encoding, "Encoding"),
    (Menu::Language, "Language"),
    (Menu::Settings, "Settings"),
    (Menu::Window, "Window"),
    (Menu::Help, "?"),
];

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowMenuState {
    pub open_window_count: usize,
}

impl WindowMenuState {
    const fn can_cycle(self) -> bool {
        self.open_window_count > 1
    }
}

#[derive(Debug, Clone, Copy)]
enum Icon {
    New,
    Open,
    Save,
    SaveAll,
    Close,
    CloseAll,
    Print,
    Cut,
    Copy,
    Paste,
    Undo,
    Redo,
    Find,
    Replace,
    ZoomIn,
    ZoomOut,
    Wrap,
    AllCharacters,
    IndentGuide,
    FunctionList,
}

pub fn menu_bar<'a>(active_menu: Option<Menu>) -> Element<'a, Message> {
    let menu_items = MENU_ITEMS
        .iter()
        .fold(row![].spacing(0), |row, (menu, label)| {
            row.push(menu_button(label, *menu, active_menu))
        });

    container(
        menu_items
            .push(space::horizontal())
            .padding(MENU_BAR_PADDING)
            .align_y(Center)
            .width(Fill),
    )
    .height(23)
    .width(Fill)
    .style(styles::menu_bar)
    .into()
}

pub fn menu_overlay<'a>(
    active_menu: Option<Menu>,
    active_path: &'a [String],
    window_menu_state: WindowMenuState,
    settings: &'a EditorSettings,
) -> Element<'a, Message> {
    let Some(menu) = active_menu else {
        return space::horizontal().into();
    };

    stack![
        mouse_area(space::vertical().height(Fill).width(Fill)).on_press(Message::MenuClosed),
        column![
            mouse_area(
                column![
                    menu_bar(active_menu),
                    row![
                        menu_prefix(menu),
                        opaque(menu_drop_down(
                            menu,
                            active_path,
                            window_menu_state,
                            settings
                        )),
                        space::horizontal(),
                    ]
                    .spacing(0)
                    .padding([0, MENU_BAR_PADDING[1]])
                    .width(Fill),
                ]
                .width(Fill)
            ),
            space::vertical(),
        ]
        .width(Fill)
        .height(Fill),
    ]
    .into()
}

fn menu_prefix<'a>(active_menu: Menu) -> Element<'a, Message> {
    MENU_ITEMS
        .iter()
        .take_while(|(menu, _)| *menu != active_menu)
        .fold(row![].spacing(0), |row, (menu, label)| {
            row.push(
                container(menu_label_content(label, *menu))
                    .padding(MENU_LABEL_PADDING)
                    .style(styles::transparent),
            )
        })
        .into()
}

pub fn tool_bar<'a>() -> Element<'a, Message> {
    let buttons = row![
        icon_button(Icon::New, "New", Message::NewFile),
        icon_button(Icon::Open, "Open", Message::OpenFile),
        icon_button(Icon::Save, "Save", Message::SaveFile),
        icon_button(Icon::SaveAll, "Save All", Message::SaveAllFiles),
        icon_button(Icon::Close, "Close", Message::CloseFile),
        icon_button(Icon::CloseAll, "Close All", Message::CloseAllFiles),
        disabled_icon_button(Icon::Print, "Print"),
        separator(),
        icon_button(Icon::Cut, "Cut", Message::Cut),
        icon_button(Icon::Copy, "Copy", Message::Copy),
        icon_button(Icon::Paste, "Paste", Message::Paste),
        separator(),
        icon_button(Icon::Undo, "Undo", Message::Undo),
        icon_button(Icon::Redo, "Redo", Message::Redo),
        separator(),
        icon_button(Icon::Find, "Find", Message::ToggleFind),
        icon_button(Icon::Replace, "Replace", Message::ShowInlineReplace),
        separator(),
        icon_button(Icon::ZoomIn, "Zoom in", Message::ZoomIn),
        icon_button(Icon::ZoomOut, "Zoom out", Message::ZoomOut),
        separator(),
        icon_button(Icon::Wrap, "Word wrap", Message::ToggleWordWrap),
        icon_button(
            Icon::AllCharacters,
            "Show all characters",
            Message::ToggleAllCharacters,
        ),
        icon_button(
            Icon::IndentGuide,
            "Indent guide",
            Message::ToggleIndentationGuides,
        ),
        separator(),
        icon_button(
            Icon::FunctionList,
            "Function List",
            Message::ToggleFunctionList,
        ),
        space::horizontal(),
    ]
    .spacing(1)
    .padding([2, 4])
    .align_y(Center)
    .width(Fill);

    container(scrollable(buttons).horizontal().width(Fill))
        .height(30)
        .width(Fill)
        .style(styles::tool_bar)
        .into()
}

fn menu_button<'a>(
    label: &'static str,
    menu: Menu,
    active_menu: Option<Menu>,
) -> Element<'a, Message> {
    mouse_area(
        container(menu_label_content(label, menu))
            .padding(MENU_LABEL_PADDING)
            .style(styles::menu_label(active_menu == Some(menu))),
    )
    .on_press(Message::MenuToggled(menu))
    .on_enter(Message::MenuHovered(menu))
    .into()
}

fn menu_label_content<'a>(label: &'static str, menu: Menu) -> Element<'a, Message> {
    if menu == Menu::Help {
        hero::icon(HeroIcon::QuestionMarkCircle, 15, IconTone::Text)
    } else {
        text(label).size(13).into()
    }
}

fn menu_drop_down<'a>(
    menu_kind: Menu,
    active_path: &'a [String],
    window_menu_state: WindowMenuState,
    settings: &'a EditorSettings,
) -> Element<'a, Message> {
    menu::view(
        menu_tree(menu_kind, window_menu_state, settings),
        active_path,
    )
}

fn menu_tree(
    menu_kind: Menu,
    window_menu_state: WindowMenuState,
    settings: &EditorSettings,
) -> MenuTree {
    match menu_kind {
        Menu::File => MenuTree::new(file_menu_entries(settings), menu_width(menu_kind)),
        Menu::Edit => MenuTree::new(edit_menu_entries(settings), menu_width(menu_kind)),
        Menu::Search => MenuTree::new(search_menu_entries(settings), menu_width(menu_kind)),
        Menu::View => MenuTree::new(view_menu_entries(settings), menu_width(menu_kind)),
        Menu::Encoding => MenuTree::new(encoding_menu_entries(), menu_width(menu_kind)),
        Menu::Language => MenuTree::new(
            highlighter::syntaxes()
                .iter()
                .map(|syntax| {
                    menu::item(
                        syntax.name.clone(),
                        Message::LanguageSelected(syntax.token.clone()),
                    )
                })
                .collect(),
            menu_width(menu_kind),
        )
        .max_height(360.0),
        Menu::Settings => MenuTree::new(
            vec![menu::item("Preferences...", Message::ToggleSettingsPanel)],
            menu_width(menu_kind),
        ),
        Menu::Window => MenuTree::new(
            window_menu_entries(window_menu_state),
            menu_width(menu_kind),
        ),
        Menu::Help => MenuTree::new(
            vec![menu::item("About Fragile Notepad", Message::AboutOpened)],
            menu_width(menu_kind),
        ),
    }
}

fn window_menu_entries(state: WindowMenuState) -> Vec<MenuNode> {
    vec![
        menu::item("Windows...", Message::WindowListOpened),
        window_cycle_item("Next Window", Message::WindowFocusNext, state),
        window_cycle_item("Previous Window", Message::WindowFocusPrevious, state),
    ]
}

fn window_cycle_item(label: &'static str, message: Message, state: WindowMenuState) -> MenuNode {
    if state.can_cycle() {
        menu::item(label, message)
    } else {
        menu::disabled(label)
    }
}

fn file_menu_entries(settings: &EditorSettings) -> Vec<MenuNode> {
    vec![
        menu_item(settings, "New", ShortcutCommand::NewFile, Message::NewFile),
        menu_item(
            settings,
            "Open...",
            ShortcutCommand::OpenFile,
            Message::OpenFile,
        ),
        menu_item(
            settings,
            "Save",
            ShortcutCommand::SaveFile,
            Message::SaveFile,
        ),
        menu_item(
            settings,
            "Save As...",
            ShortcutCommand::SaveFileAs,
            Message::SaveFileAs,
        ),
        menu::disabled("Save a Copy As..."),
        menu::item("Save All", Message::SaveAllFiles),
        menu::item("Close", Message::CloseFile),
        menu::item("Close All", Message::CloseAllFiles),
        close_multiple_documents_menu(),
    ]
}

fn edit_menu_entries(settings: &EditorSettings) -> Vec<MenuNode> {
    vec![
        menu_item(settings, "Undo", ShortcutCommand::Undo, Message::Undo),
        menu_item(settings, "Redo", ShortcutCommand::Redo, Message::Redo),
        menu::separator(),
        menu_item(settings, "Cut", ShortcutCommand::Cut, Message::Cut),
        menu_item(settings, "Copy", ShortcutCommand::Copy, Message::Copy),
        menu_item(settings, "Paste", ShortcutCommand::Paste, Message::Paste),
        menu::item_with_shortcut("Delete", "Del", Message::Delete),
        menu_item(
            settings,
            "Select All",
            ShortcutCommand::SelectAll,
            Message::Shortcut(ShortcutCommand::SelectAll),
        ),
        menu::separator(),
        selection_operations_menu(settings),
        menu::separator(),
        indent_menu(settings),
        menu::separator(),
        line_operations_menu(settings),
    ]
}

fn search_menu_entries(settings: &EditorSettings) -> Vec<MenuNode> {
    let mut entries = vec![
        menu_item(
            settings,
            "Find...",
            ShortcutCommand::ToggleFind,
            Message::ToggleFind,
        ),
        menu::item(
            "Find in Files...",
            Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::FindInFiles),
        ),
        menu::item("Find Next", Message::FindNext),
        menu::item("Find Previous", Message::FindPrevious),
        menu::item("Select and Find Next", Message::SelectAndFindNext),
        menu::item("Select and Find Previous", Message::SelectAndFindPrevious),
        menu::item("Find (Volatile) Next", Message::VolatileFindNext),
        menu::item("Find (Volatile) Previous", Message::VolatileFindPrevious),
        menu::item("Replace...", Message::ShowInlineReplace),
        menu_item(
            settings,
            "Advanced Find...",
            ShortcutCommand::AdvancedFind,
            Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::Find),
        ),
        menu_item(
            settings,
            "Advanced Replace...",
            ShortcutCommand::AdvancedReplace,
            Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::Replace),
        ),
        menu::item(
            "Replace in Open Documents...",
            Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::ReplaceInFiles),
        ),
        menu::separator(),
    ];

    entries.extend(matching_and_function_items(settings));
    entries
}

fn view_menu_entries(settings: &EditorSettings) -> Vec<MenuNode> {
    vec![
        menu::item("Word Wrap", Message::ToggleWordWrap),
        menu::item("Line Numbers", Message::ToggleLineNumbers),
        show_symbol_menu(),
        menu::item("Function List", Message::ToggleFunctionList),
        zoom_menu(settings),
        menu::separator(),
        fold_commands_menu(settings),
        menu::item("Folding Controls", Message::ToggleFoldingControls),
    ]
}

fn menu_item(
    settings: &EditorSettings,
    label: impl Into<String>,
    command: ShortcutCommand,
    message: Message,
) -> MenuNode {
    if let Some(binding) = settings.shortcuts.binding(command) {
        menu::item_with_shortcut_binding(label, binding, message)
    } else {
        menu::item(label, message)
    }
}

fn close_multiple_documents_menu() -> MenuNode {
    menu::submenu(
        "close-multiple",
        "Close Multiple Documents",
        vec![
            menu::item(
                "Close All but Active Document",
                Message::CloseAllButActiveFile,
            ),
            menu::item(
                "Close All but Pinned Documents",
                Message::CloseAllButPinnedFiles,
            ),
            menu::item("Close All to the Left", Message::CloseAllToLeft),
            menu::item("Close All to the Right", Message::CloseAllToRight),
            menu::item("Close All Unchanged", Message::CloseAllUnchanged),
        ],
    )
}

fn indent_menu(settings: &EditorSettings) -> MenuNode {
    menu::submenu(
        "indent",
        "Indent",
        vec![
            menu_item(
                settings,
                "Increase Line Indent",
                ShortcutCommand::Indent,
                Message::Shortcut(ShortcutCommand::Indent),
            ),
            menu_item(
                settings,
                "Decrease Line Indent",
                ShortcutCommand::Unindent,
                Message::Shortcut(ShortcutCommand::Unindent),
            ),
        ],
    )
}

fn selection_operations_menu(settings: &EditorSettings) -> MenuNode {
    menu::submenu(
        "selection-operations",
        "Selection",
        vec![
            menu_item(
                settings,
                "Add Caret Above",
                ShortcutCommand::AddCaretAbove,
                Message::Shortcut(ShortcutCommand::AddCaretAbove),
            ),
            menu_item(
                settings,
                "Add Caret Below",
                ShortcutCommand::AddCaretBelow,
                Message::Shortcut(ShortcutCommand::AddCaretBelow),
            ),
            menu_item(
                settings,
                "Split Selection into Lines",
                ShortcutCommand::SplitSelectionIntoLines,
                Message::Shortcut(ShortcutCommand::SplitSelectionIntoLines),
            ),
            menu_item(
                settings,
                "Convert Selection to Rectangle",
                ShortcutCommand::ConvertSelectionToRectangle,
                Message::Shortcut(ShortcutCommand::ConvertSelectionToRectangle),
            ),
        ],
    )
}

fn line_operations_menu(settings: &EditorSettings) -> MenuNode {
    menu::submenu(
        "line-operations",
        "Line Operations",
        vec![
            menu_item(
                settings,
                "Duplicate Current Line",
                ShortcutCommand::DuplicateLine,
                Message::Shortcut(ShortcutCommand::DuplicateLine),
            ),
            menu_item(
                settings,
                "Delete Current Line",
                ShortcutCommand::DeleteLine,
                Message::Shortcut(ShortcutCommand::DeleteLine),
            ),
            menu_item(
                settings,
                "Copy Current Line",
                ShortcutCommand::CopyLine,
                Message::Shortcut(ShortcutCommand::CopyLine),
            ),
            menu_item(
                settings,
                "Cut Current Line",
                ShortcutCommand::CutLine,
                Message::Shortcut(ShortcutCommand::CutLine),
            ),
        ],
    )
}

fn matching_and_function_items(settings: &EditorSettings) -> Vec<MenuNode> {
    vec![
        menu_item(
            settings,
            "Go to Matching Brace",
            ShortcutCommand::GoToMatchingDelimiter,
            Message::GoToMatchingDelimiter,
        ),
        menu_item(
            settings,
            "Select All In-between {} [] or ()",
            ShortcutCommand::SelectMatchingDelimiter,
            Message::SelectMatchingDelimiter,
        ),
        menu_item(
            settings,
            "Next Function",
            ShortcutCommand::NextFunction,
            Message::NextFunction,
        ),
        menu_item(
            settings,
            "Previous Function",
            ShortcutCommand::PreviousFunction,
            Message::PreviousFunction,
        ),
        menu_item(
            settings,
            "Select Current Function",
            ShortcutCommand::SelectCurrentFunction,
            Message::SelectCurrentFunction,
        ),
        menu_item(
            settings,
            "Select Current Function Body",
            ShortcutCommand::SelectCurrentFunctionBody,
            Message::SelectCurrentFunctionBody,
        ),
    ]
}

fn show_symbol_menu() -> MenuNode {
    menu::submenu(
        "show-symbol",
        "Show Symbol",
        vec![
            menu::item("Show Space and Tab", Message::ToggleSpaceAndTab),
            menu::item("Show End of Line", Message::ToggleEolMarkers),
            menu::disabled("Show Non-Printing Characters"),
            menu::disabled("Show Control Characters && Unicode EOL"),
            menu::item("Show All Characters", Message::ToggleAllCharacters),
            menu::separator(),
            menu::item("Show Indent Guide", Message::ToggleIndentationGuides),
            menu::disabled("Show Wrap Symbol"),
        ],
    )
}

fn zoom_menu(settings: &EditorSettings) -> MenuNode {
    menu::submenu(
        "zoom",
        "Zoom",
        vec![
            menu_item(
                settings,
                "Zoom In (Ctrl+Mouse Wheel Up)",
                ShortcutCommand::ZoomIn,
                Message::ZoomIn,
            ),
            menu_item(
                settings,
                "Zoom Out (Ctrl+Mouse Wheel Down)",
                ShortcutCommand::ZoomOut,
                Message::ZoomOut,
            ),
            menu_item(
                settings,
                "Restore Default Zoom",
                ShortcutCommand::ZoomReset,
                Message::ZoomReset,
            ),
        ],
    )
}

fn fold_commands_menu(settings: &EditorSettings) -> MenuNode {
    menu::submenu(
        "fold",
        "Fold",
        vec![
            menu_item(
                settings,
                "Fold All",
                ShortcutCommand::FoldAll,
                Message::FoldAll,
            ),
            menu_item(
                settings,
                "Unfold All",
                ShortcutCommand::UnfoldAll,
                Message::UnfoldAll,
            ),
            menu_item(
                settings,
                "Fold Current Level",
                ShortcutCommand::FoldCurrent,
                Message::FoldCurrent,
            ),
            menu_item(
                settings,
                "Unfold Current Level",
                ShortcutCommand::UnfoldCurrent,
                Message::UnfoldCurrent,
            ),
            menu_item(
                settings,
                "Toggle Current Fold",
                ShortcutCommand::ToggleCurrentFold,
                Message::ToggleCurrentFold,
            ),
        ],
    )
}

fn encoding_menu_entries() -> Vec<MenuNode> {
    vec![
        menu::item("ANSI", Message::EncodingSelected(TextEncoding::Windows1252)),
        menu::item("UTF-8", Message::EncodingSelected(TextEncoding::Utf8)),
        menu::item(
            "UTF-8-BOM",
            Message::EncodingSelected(TextEncoding::Utf8Bom),
        ),
        menu::item(
            "UTF-16 BE BOM",
            Message::EncodingSelected(TextEncoding::Utf16BeBom),
        ),
        menu::item(
            "UTF-16 LE BOM",
            Message::EncodingSelected(TextEncoding::Utf16LeBom),
        ),
        menu::submenu(
            "character-sets",
            "Character sets",
            encoding_character_sets(),
        ),
        menu::separator(),
        menu::item(
            "Convert to ANSI",
            Message::EncodingSelected(TextEncoding::Windows1252),
        ),
        menu::item(
            "Convert to UTF-8",
            Message::EncodingSelected(TextEncoding::Utf8),
        ),
        menu::item(
            "Convert to UTF-8-BOM",
            Message::EncodingSelected(TextEncoding::Utf8Bom),
        ),
        menu::item(
            "Convert to UTF-16 BE BOM",
            Message::EncodingSelected(TextEncoding::Utf16BeBom),
        ),
        menu::item(
            "Convert to UTF-16 LE BOM",
            Message::EncodingSelected(TextEncoding::Utf16LeBom),
        ),
    ]
}

fn encoding_character_sets() -> Vec<MenuNode> {
    vec![
        menu::submenu(
            "arabic",
            "Arabic",
            vec![
                menu::item(
                    "ISO 8859-6",
                    Message::EncodingSelected(TextEncoding::Iso8859_6),
                ),
                menu::item("OEM 720", Message::EncodingSelected(TextEncoding::Oem720)),
                menu::item(
                    "Windows-1256",
                    Message::EncodingSelected(TextEncoding::Windows1256),
                ),
            ],
        ),
        menu::submenu(
            "baltic",
            "Baltic",
            vec![
                menu::item(
                    "ISO 8859-4",
                    Message::EncodingSelected(TextEncoding::Iso8859_4),
                ),
                menu::item(
                    "ISO 8859-13",
                    Message::EncodingSelected(TextEncoding::Iso8859_13),
                ),
                menu::item("OEM 775", Message::EncodingSelected(TextEncoding::Oem775)),
                menu::item(
                    "Windows-1257",
                    Message::EncodingSelected(TextEncoding::Windows1257),
                ),
            ],
        ),
        menu::submenu(
            "celtic",
            "Celtic",
            vec![menu::item(
                "ISO 8859-14",
                Message::EncodingSelected(TextEncoding::Iso8859_14),
            )],
        ),
        menu::submenu(
            "cyrillic",
            "Cyrillic",
            vec![
                menu::item(
                    "ISO 8859-5",
                    Message::EncodingSelected(TextEncoding::Iso8859_5),
                ),
                menu::item("KOI8-R", Message::EncodingSelected(TextEncoding::Koi8R)),
                menu::item("KOI8-U", Message::EncodingSelected(TextEncoding::Koi8U)),
                menu::item(
                    "Macintosh",
                    Message::EncodingSelected(TextEncoding::Macintosh),
                ),
                menu::item("OEM 855", Message::EncodingSelected(TextEncoding::Oem855)),
                menu::item("OEM 866", Message::EncodingSelected(TextEncoding::Oem866)),
                menu::item(
                    "Windows-1251",
                    Message::EncodingSelected(TextEncoding::Windows1251),
                ),
            ],
        ),
        menu::submenu(
            "central-european",
            "Central European",
            vec![
                menu::item("OEM 852", Message::EncodingSelected(TextEncoding::Oem852)),
                menu::item(
                    "Windows-1250",
                    Message::EncodingSelected(TextEncoding::Windows1250),
                ),
            ],
        ),
        menu::submenu(
            "chinese",
            "Chinese",
            vec![
                menu::item(
                    "Big5 (Traditional)",
                    Message::EncodingSelected(TextEncoding::Big5),
                ),
                menu::item(
                    "GB2312 (Simplified)",
                    Message::EncodingSelected(TextEncoding::Gb18030),
                ),
            ],
        ),
        menu::submenu(
            "eastern-european",
            "Eastern European",
            vec![menu::item(
                "ISO 8859-2",
                Message::EncodingSelected(TextEncoding::Iso8859_2),
            )],
        ),
        menu::submenu(
            "greek",
            "Greek",
            vec![
                menu::item(
                    "ISO 8859-7",
                    Message::EncodingSelected(TextEncoding::Iso8859_7),
                ),
                menu::item("OEM 737", Message::EncodingSelected(TextEncoding::Oem737)),
                menu::item("OEM 869", Message::EncodingSelected(TextEncoding::Oem869)),
                menu::item(
                    "Windows-1253",
                    Message::EncodingSelected(TextEncoding::Windows1253),
                ),
            ],
        ),
        menu::submenu(
            "hebrew",
            "Hebrew",
            vec![
                menu::item(
                    "ISO 8859-8",
                    Message::EncodingSelected(TextEncoding::Iso8859_8),
                ),
                menu::item("OEM 862", Message::EncodingSelected(TextEncoding::Oem862)),
                menu::item(
                    "Windows-1255",
                    Message::EncodingSelected(TextEncoding::Windows1255),
                ),
            ],
        ),
        menu::submenu(
            "japanese",
            "Japanese",
            vec![menu::item(
                "Shift-JIS",
                Message::EncodingSelected(TextEncoding::ShiftJis),
            )],
        ),
        menu::submenu(
            "korean",
            "Korean",
            vec![
                menu::item(
                    "Windows 949",
                    Message::EncodingSelected(TextEncoding::EucKr),
                ),
                menu::item("EUC-KR", Message::EncodingSelected(TextEncoding::EucKr)),
            ],
        ),
        menu::submenu(
            "north-european",
            "North European",
            vec![
                menu::item(
                    "OEM 861 : Icelandic",
                    Message::EncodingSelected(TextEncoding::Oem861),
                ),
                menu::item(
                    "OEM 865 : Nordic",
                    Message::EncodingSelected(TextEncoding::Oem865),
                ),
            ],
        ),
        menu::submenu(
            "thai",
            "Thai",
            vec![menu::item(
                "TIS-620",
                Message::EncodingSelected(TextEncoding::Tis620),
            )],
        ),
        menu::submenu(
            "turkish",
            "Turkish",
            vec![
                menu::item(
                    "ISO 8859-3",
                    Message::EncodingSelected(TextEncoding::Iso8859_3),
                ),
                menu::item(
                    "ISO 8859-9",
                    Message::EncodingSelected(TextEncoding::Iso8859_9),
                ),
                menu::item("OEM 857", Message::EncodingSelected(TextEncoding::Oem857)),
                menu::item(
                    "Windows-1254",
                    Message::EncodingSelected(TextEncoding::Windows1254),
                ),
            ],
        ),
        menu::submenu(
            "western-european",
            "Western European",
            vec![
                menu::item(
                    "ISO 8859-1",
                    Message::EncodingSelected(TextEncoding::Iso8859_1),
                ),
                menu::item(
                    "ISO 8859-15",
                    Message::EncodingSelected(TextEncoding::Iso8859_15),
                ),
                menu::item("OEM 850", Message::EncodingSelected(TextEncoding::Oem850)),
                menu::item("OEM 858", Message::EncodingSelected(TextEncoding::Oem858)),
                menu::item(
                    "OEM 860 : Portuguese",
                    Message::EncodingSelected(TextEncoding::Oem860),
                ),
                menu::item(
                    "OEM 863 : French",
                    Message::EncodingSelected(TextEncoding::Oem863),
                ),
                menu::item("OEM-US", Message::EncodingSelected(TextEncoding::Oem437)),
                menu::item(
                    "Windows-1252",
                    Message::EncodingSelected(TextEncoding::Windows1252),
                ),
            ],
        ),
        menu::submenu(
            "vietnamese",
            "Vietnamese",
            vec![menu::item(
                "Windows-1258",
                Message::EncodingSelected(TextEncoding::Windows1258),
            )],
        ),
    ]
}

fn menu_width(menu: Menu) -> f32 {
    match menu {
        Menu::Language => 288.0,
        Menu::Encoding => 260.0,
        Menu::Window => 244.0,
        _ => 202.0,
    }
}

fn icon_button<'a>(icon: Icon, label: &'static str, message: Message) -> Element<'a, Message> {
    toolbar_button(icon, label, Some(message))
}

fn disabled_icon_button<'a>(icon: Icon, label: &'static str) -> Element<'a, Message> {
    toolbar_button(icon, label, None)
}

fn toolbar_button<'a>(
    icon: Icon,
    label: &'static str,
    message: Option<Message>,
) -> Element<'a, Message> {
    tooltip(
        button(centered_button_content(
            image::Image::new(icon_handle(icon))
                .width(18)
                .height(18)
                .filter_method(image::FilterMethod::Linear),
        ))
        .width(25)
        .height(24)
        .padding(3)
        .style(styles::icon_button)
        .on_press_maybe(message),
        container(text(label).size(12))
            .padding([4, 7])
            .style(styles::tooltip),
        tooltip::Position::Bottom,
    )
    .gap(4)
    .into()
}

fn separator<'a>() -> Element<'a, Message> {
    container(space::horizontal().width(1))
        .height(22)
        .width(Length::Fixed(5.0))
        .padding([2, 2])
        .style(styles::separator)
        .into()
}

fn icon_handle(icon: Icon) -> image::Handle {
    tango::handle(match icon {
        Icon::New => TangoIcon::New,
        Icon::Open => TangoIcon::Open,
        Icon::Save => TangoIcon::Save,
        Icon::SaveAll => TangoIcon::SaveAll,
        Icon::Close => TangoIcon::Close,
        Icon::CloseAll => TangoIcon::CloseAll,
        Icon::Print => TangoIcon::Print,
        Icon::Cut => TangoIcon::Cut,
        Icon::Copy => TangoIcon::Copy,
        Icon::Paste => TangoIcon::Paste,
        Icon::Undo => TangoIcon::Undo,
        Icon::Redo => TangoIcon::Redo,
        Icon::Find => TangoIcon::Find,
        Icon::Replace => TangoIcon::Replace,
        Icon::ZoomIn => TangoIcon::ZoomIn,
        Icon::ZoomOut => TangoIcon::ZoomOut,
        Icon::Wrap => TangoIcon::WordWrap,
        Icon::AllCharacters => TangoIcon::AllCharacters,
        Icon::IndentGuide => TangoIcon::IndentGuide,
        Icon::FunctionList => TangoIcon::FunctionList,
    })
}
