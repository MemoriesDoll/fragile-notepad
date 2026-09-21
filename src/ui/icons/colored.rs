use iced::advanced::image;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColoredIcon {
    AllCharacters,
    Close,
    CloseAll,
    Copy,
    Cut,
    Delete,
    DocumentSaved,
    DocumentUnsaved,
    Find,
    FunctionList,
    IndentGuide,
    New,
    Open,
    Paste,
    Print,
    Redo,
    Replace,
    Save,
    SaveAll,
    TabClose,
    Undo,
    WordWrap,
    ZoomIn,
    ZoomOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColoredIconAsset {
    AccessoriesCharacterMap,
    DocumentClose,
    DocumentCloseAll,
    DocumentNew,
    DocumentOpen,
    DocumentPrint,
    DocumentSave,
    DocumentSaveAll,
    DocumentSaveAs,
    EditCopy,
    EditCut,
    EditDelete,
    EditFind,
    EditFindReplace,
    EditPaste,
    EditRedo,
    EditUndo,
    EmblemFavorite,
    EmblemImportant,
    FormatIndentMore,
    FormatJustifyFill,
    ProcessStop,
    TabClose,
    TabDocumentMonitoring,
    TabDocumentReadOnly,
    TabDocumentSaved,
    TabDocumentSystemReadOnly,
    TabDocumentUnsaved,
    TextXGeneric,
    TextXGenericTemplate,
    TextXScript,
    ZoomIn,
    ZoomOut,
}

impl ColoredIconAsset {
    pub fn rgba_bytes(self) -> &'static [u8] {
        match self {
            ColoredIconAsset::AccessoriesCharacterMap => {
                include_bytes!("../../../assets/icons/colored/rgba/accessories-character-map.rgba")
            }
            ColoredIconAsset::DocumentClose => {
                include_bytes!("../../../assets/icons/colored/rgba/document-close.rgba")
            }
            ColoredIconAsset::DocumentCloseAll => {
                include_bytes!("../../../assets/icons/colored/rgba/document-close-all.rgba")
            }
            ColoredIconAsset::DocumentNew => {
                include_bytes!("../../../assets/icons/colored/rgba/document-new.rgba")
            }
            ColoredIconAsset::DocumentOpen => {
                include_bytes!("../../../assets/icons/colored/rgba/document-open.rgba")
            }
            ColoredIconAsset::DocumentPrint => {
                include_bytes!("../../../assets/icons/colored/rgba/document-print.rgba")
            }
            ColoredIconAsset::DocumentSave => {
                include_bytes!("../../../assets/icons/colored/rgba/document-save.rgba")
            }
            ColoredIconAsset::DocumentSaveAll => {
                include_bytes!("../../../assets/icons/colored/rgba/document-save-all.rgba")
            }
            ColoredIconAsset::DocumentSaveAs => {
                include_bytes!("../../../assets/icons/colored/rgba/document-save-as.rgba")
            }
            ColoredIconAsset::EditCopy => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-copy.rgba")
            }
            ColoredIconAsset::EditCut => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-cut.rgba")
            }
            ColoredIconAsset::EditDelete => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-delete.rgba")
            }
            ColoredIconAsset::EditFind => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-find.rgba")
            }
            ColoredIconAsset::EditFindReplace => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-find-replace.rgba")
            }
            ColoredIconAsset::EditPaste => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-paste.rgba")
            }
            ColoredIconAsset::EditRedo => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-redo.rgba")
            }
            ColoredIconAsset::EditUndo => {
                include_bytes!("../../../assets/icons/colored/rgba/edit-undo.rgba")
            }
            ColoredIconAsset::EmblemFavorite => {
                include_bytes!("../../../assets/icons/colored/rgba/emblem-favorite.rgba")
            }
            ColoredIconAsset::EmblemImportant => {
                include_bytes!("../../../assets/icons/colored/rgba/emblem-important.rgba")
            }
            ColoredIconAsset::FormatIndentMore => {
                include_bytes!("../../../assets/icons/colored/rgba/format-indent-more.rgba")
            }
            ColoredIconAsset::FormatJustifyFill => {
                include_bytes!("../../../assets/icons/colored/rgba/format-justify-fill.rgba")
            }
            ColoredIconAsset::ProcessStop => {
                include_bytes!("../../../assets/icons/colored/rgba/process-stop.rgba")
            }
            ColoredIconAsset::TabClose => {
                include_bytes!("../../../assets/icons/colored/rgba/tab-close.rgba")
            }
            ColoredIconAsset::TabDocumentMonitoring => {
                include_bytes!("../../../assets/icons/colored/rgba/tab-document-monitoring.rgba")
            }
            ColoredIconAsset::TabDocumentReadOnly => {
                include_bytes!("../../../assets/icons/colored/rgba/tab-document-read-only.rgba")
            }
            ColoredIconAsset::TabDocumentSaved => {
                include_bytes!("../../../assets/icons/colored/rgba/tab-document-saved.rgba")
            }
            ColoredIconAsset::TabDocumentSystemReadOnly => {
                include_bytes!(
                    "../../../assets/icons/colored/rgba/tab-document-system-read-only.rgba"
                )
            }
            ColoredIconAsset::TabDocumentUnsaved => {
                include_bytes!("../../../assets/icons/colored/rgba/tab-document-unsaved.rgba")
            }
            ColoredIconAsset::TextXGeneric => {
                include_bytes!("../../../assets/icons/colored/rgba/text-x-generic.rgba")
            }
            ColoredIconAsset::TextXGenericTemplate => {
                include_bytes!("../../../assets/icons/colored/rgba/text-x-generic-template.rgba")
            }
            ColoredIconAsset::TextXScript => {
                include_bytes!("../../../assets/icons/colored/rgba/text-x-script.rgba")
            }
            ColoredIconAsset::ZoomIn => {
                include_bytes!("../../../assets/icons/colored/rgba/zoom-in.rgba")
            }
            ColoredIconAsset::ZoomOut => {
                include_bytes!("../../../assets/icons/colored/rgba/zoom-out.rgba")
            }
        }
    }
}

pub fn handle(icon: ColoredIcon) -> image::Handle {
    let asset = asset(icon);

    static CACHE: LazyLock<Mutex<HashMap<ColoredIconAsset, image::Handle>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let mut cache = CACHE.lock().expect("colored icon cache");
    cache
        .entry(asset)
        .or_insert_with(|| {
            image::Handle::from_rgba(
                super::ICON_SIZE,
                super::ICON_SIZE,
                asset.rgba_bytes().to_vec(),
            )
        })
        .clone()
}

fn asset(icon: ColoredIcon) -> ColoredIconAsset {
    match icon {
        ColoredIcon::AllCharacters => ColoredIconAsset::AccessoriesCharacterMap,
        ColoredIcon::Close => ColoredIconAsset::DocumentClose,
        ColoredIcon::CloseAll => ColoredIconAsset::DocumentCloseAll,
        ColoredIcon::Delete => ColoredIconAsset::EditDelete,
        ColoredIcon::Copy => ColoredIconAsset::EditCopy,
        ColoredIcon::Cut => ColoredIconAsset::EditCut,
        ColoredIcon::DocumentSaved => ColoredIconAsset::TabDocumentSaved,
        ColoredIcon::DocumentUnsaved => ColoredIconAsset::TabDocumentUnsaved,
        ColoredIcon::Find => ColoredIconAsset::EditFind,
        ColoredIcon::FunctionList => ColoredIconAsset::TextXScript,
        ColoredIcon::IndentGuide => ColoredIconAsset::FormatIndentMore,
        ColoredIcon::New => ColoredIconAsset::DocumentNew,
        ColoredIcon::Open => ColoredIconAsset::DocumentOpen,
        ColoredIcon::Paste => ColoredIconAsset::EditPaste,
        ColoredIcon::Print => ColoredIconAsset::DocumentPrint,
        ColoredIcon::Redo => ColoredIconAsset::EditRedo,
        ColoredIcon::Replace => ColoredIconAsset::EditFindReplace,
        ColoredIcon::Save => ColoredIconAsset::DocumentSave,
        ColoredIcon::SaveAll => ColoredIconAsset::DocumentSaveAll,
        ColoredIcon::TabClose => ColoredIconAsset::TabClose,
        ColoredIcon::Undo => ColoredIconAsset::EditUndo,
        ColoredIcon::WordWrap => ColoredIconAsset::FormatJustifyFill,
        ColoredIcon::ZoomIn => ColoredIconAsset::ZoomIn,
        ColoredIcon::ZoomOut => ColoredIconAsset::ZoomOut,
    }
}
