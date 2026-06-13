use iced::advanced::image;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TangoIcon {
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
pub enum TangoIconAsset {
    AccessoriesCharacterMap,
    DocumentNew,
    DocumentOpen,
    DocumentPrint,
    DocumentSave,
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

impl TangoIconAsset {
    pub fn rgba_bytes(self) -> &'static [u8] {
        match self {
            TangoIconAsset::AccessoriesCharacterMap => {
                include_bytes!("../../../assets/icons/tango/rgba/accessories-character-map.rgba")
            }
            TangoIconAsset::DocumentNew => {
                include_bytes!("../../../assets/icons/tango/rgba/document-new.rgba")
            }
            TangoIconAsset::DocumentOpen => {
                include_bytes!("../../../assets/icons/tango/rgba/document-open.rgba")
            }
            TangoIconAsset::DocumentPrint => {
                include_bytes!("../../../assets/icons/tango/rgba/document-print.rgba")
            }
            TangoIconAsset::DocumentSave => {
                include_bytes!("../../../assets/icons/tango/rgba/document-save.rgba")
            }
            TangoIconAsset::DocumentSaveAs => {
                include_bytes!("../../../assets/icons/tango/rgba/document-save-as.rgba")
            }
            TangoIconAsset::EditCopy => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-copy.rgba")
            }
            TangoIconAsset::EditCut => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-cut.rgba")
            }
            TangoIconAsset::EditDelete => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-delete.rgba")
            }
            TangoIconAsset::EditFind => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-find.rgba")
            }
            TangoIconAsset::EditFindReplace => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-find-replace.rgba")
            }
            TangoIconAsset::EditPaste => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-paste.rgba")
            }
            TangoIconAsset::EditRedo => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-redo.rgba")
            }
            TangoIconAsset::EditUndo => {
                include_bytes!("../../../assets/icons/tango/rgba/edit-undo.rgba")
            }
            TangoIconAsset::EmblemFavorite => {
                include_bytes!("../../../assets/icons/tango/rgba/emblem-favorite.rgba")
            }
            TangoIconAsset::EmblemImportant => {
                include_bytes!("../../../assets/icons/tango/rgba/emblem-important.rgba")
            }
            TangoIconAsset::FormatIndentMore => {
                include_bytes!("../../../assets/icons/tango/rgba/format-indent-more.rgba")
            }
            TangoIconAsset::FormatJustifyFill => {
                include_bytes!("../../../assets/icons/tango/rgba/format-justify-fill.rgba")
            }
            TangoIconAsset::ProcessStop => {
                include_bytes!("../../../assets/icons/tango/rgba/process-stop.rgba")
            }
            TangoIconAsset::TabClose => {
                include_bytes!("../../../assets/icons/tango/rgba/tab-close.rgba")
            }
            TangoIconAsset::TabDocumentMonitoring => {
                include_bytes!("../../../assets/icons/tango/rgba/tab-document-monitoring.rgba")
            }
            TangoIconAsset::TabDocumentReadOnly => {
                include_bytes!("../../../assets/icons/tango/rgba/tab-document-read-only.rgba")
            }
            TangoIconAsset::TabDocumentSaved => {
                include_bytes!("../../../assets/icons/tango/rgba/tab-document-saved.rgba")
            }
            TangoIconAsset::TabDocumentSystemReadOnly => {
                include_bytes!(
                    "../../../assets/icons/tango/rgba/tab-document-system-read-only.rgba"
                )
            }
            TangoIconAsset::TabDocumentUnsaved => {
                include_bytes!("../../../assets/icons/tango/rgba/tab-document-unsaved.rgba")
            }
            TangoIconAsset::TextXGeneric => {
                include_bytes!("../../../assets/icons/tango/rgba/text-x-generic.rgba")
            }
            TangoIconAsset::TextXGenericTemplate => {
                include_bytes!("../../../assets/icons/tango/rgba/text-x-generic-template.rgba")
            }
            TangoIconAsset::TextXScript => {
                include_bytes!("../../../assets/icons/tango/rgba/text-x-script.rgba")
            }
            TangoIconAsset::ZoomIn => {
                include_bytes!("../../../assets/icons/tango/rgba/zoom-in.rgba")
            }
            TangoIconAsset::ZoomOut => {
                include_bytes!("../../../assets/icons/tango/rgba/zoom-out.rgba")
            }
        }
    }
}

pub fn handle(icon: TangoIcon) -> image::Handle {
    let asset = asset(icon);

    static CACHE: LazyLock<Mutex<HashMap<TangoIconAsset, image::Handle>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let mut cache = CACHE.lock().expect("tango icon cache");
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

fn asset(icon: TangoIcon) -> TangoIconAsset {
    match icon {
        TangoIcon::AllCharacters => TangoIconAsset::AccessoriesCharacterMap,
        TangoIcon::Close => TangoIconAsset::ProcessStop,
        TangoIcon::CloseAll | TangoIcon::Delete => TangoIconAsset::EditDelete,
        TangoIcon::Copy => TangoIconAsset::EditCopy,
        TangoIcon::Cut => TangoIconAsset::EditCut,
        TangoIcon::DocumentSaved => TangoIconAsset::TabDocumentSaved,
        TangoIcon::DocumentUnsaved => TangoIconAsset::TabDocumentUnsaved,
        TangoIcon::Find => TangoIconAsset::EditFind,
        TangoIcon::FunctionList => TangoIconAsset::TextXScript,
        TangoIcon::IndentGuide => TangoIconAsset::FormatIndentMore,
        TangoIcon::New => TangoIconAsset::DocumentNew,
        TangoIcon::Open => TangoIconAsset::DocumentOpen,
        TangoIcon::Paste => TangoIconAsset::EditPaste,
        TangoIcon::Print => TangoIconAsset::DocumentPrint,
        TangoIcon::Redo => TangoIconAsset::EditRedo,
        TangoIcon::Replace => TangoIconAsset::EditFindReplace,
        TangoIcon::Save => TangoIconAsset::DocumentSave,
        TangoIcon::SaveAll => TangoIconAsset::DocumentSaveAs,
        TangoIcon::TabClose => TangoIconAsset::TabClose,
        TangoIcon::Undo => TangoIconAsset::EditUndo,
        TangoIcon::WordWrap => TangoIconAsset::FormatJustifyFill,
        TangoIcon::ZoomIn => TangoIconAsset::ZoomIn,
        TangoIcon::ZoomOut => TangoIconAsset::ZoomOut,
    }
}
