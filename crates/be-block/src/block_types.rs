use uuid::Uuid;

use crate::{
    AudioContent, BeuiDemoContent, BlockContent, BrowserTabContent, CalendarContent, CanvasContent,
    ChecklistContent, CheckoutContent, CompiledLogicContent, CounterContent, DatabaseContent,
    DatabaseSchemaContent, DatabaseViewContent, DeterministicGameContent, DisplaySettingsContent,
    EditorSettingsContent, EditorViewContent, FolderContent, GameModuleContent, HotbarContent,
    ImageContent, InputRemapperContent, InputSettingsContent, LogicGameContent, LogicGridContent,
    MapContent, PanZoomContent, PdfContent, PixelArtContent, PixelRayTracerContent,
    PresentationContent, RepositoryContent, Scene3dContent, SettingsContent,
    SettingsProfileContent, TextContent, TriangleContent, VideoContent, WorkspaceContent,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChildEdits {
    pub add: bool,
    pub delete: bool,
    pub replace: bool,
}

impl ChildEdits {
    pub const NONE: Self = Self {
        add: false,
        delete: false,
        replace: false,
    };
    pub const REPLACE: Self = Self {
        add: false,
        delete: false,
        replace: true,
    };
    pub const ALL: Self = Self {
        add: true,
        delete: true,
        replace: true,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockType {
    pub content_type: Uuid,
    pub name: &'static str,
    pub icon: &'static str,
    pub color: [u8; 3],
    pub children: ChildEdits,
}

const fn of<C: BlockContent>(
    name: &'static str,
    icon: &'static str,
    color: [u8; 3],
    children: ChildEdits,
) -> BlockType {
    BlockType {
        content_type: C::CONTENT_TYPE,
        name,
        icon,
        color,
        children,
    }
}

const RED: [u8; 3] = [203, 91, 125];
const TEAL: [u8; 3] = [64, 155, 173];
const GOLD: [u8; 3] = [172, 145, 47];
const BLUE: [u8; 3] = [65, 145, 210];
const GREEN: [u8; 3] = [44, 158, 128];
const PURPLE: [u8; 3] = [155, 112, 211];
const ORANGE: [u8; 3] = [211, 126, 59];

pub const BLOCK_TYPES: &[BlockType] = &[
    of::<AudioContent>("Audio", "\u{eb82}", BLUE, ChildEdits::NONE),
    of::<BeuiDemoContent>("beui demo", "\u{e1bd}", GREEN, ChildEdits::NONE),
    of::<BrowserTabContent>("Web Browser Tab", "\u{e894}", PURPLE, ChildEdits::NONE),
    of::<CalendarContent>("Calendar", "\u{ebcc}", ORANGE, ChildEdits::NONE),
    of::<CanvasContent>("Canvas", "\u{e746}", GOLD, ChildEdits::REPLACE),
    of::<ChecklistContent>("Checklist", "\u{e6b1}", RED, ChildEdits::NONE),
    of::<CheckoutContent>("Checkout", "\u{e97a}", BLUE, ChildEdits::NONE),
    of::<CompiledLogicContent>("Compiled Logic", "\u{e322}", RED, ChildEdits::NONE),
    of::<CounterContent>("Counter", "\u{eb8d}", TEAL, ChildEdits::NONE),
    of::<DatabaseContent>("Database", "\u{f20e}", GOLD, ChildEdits::NONE),
    of::<DatabaseSchemaContent>("Database Schema", "\u{e4fd}", BLUE, ChildEdits::NONE),
    of::<DatabaseViewContent>("Database View", "\u{f1be}", GREEN, ChildEdits::NONE),
    of::<DeterministicGameContent>("Game", "\u{f015}", PURPLE, ChildEdits::NONE),
    of::<DisplaySettingsContent>("Display", "\u{eb97}", BLUE, ChildEdits::NONE),
    of::<EditorSettingsContent>("Default editors", "\u{eb97}", ORANGE, ChildEdits::NONE),
    of::<EditorViewContent>("View", "\u{e8f4}", TEAL, ChildEdits::NONE),
    of::<FolderContent>("Folder", "\u{e2c7}", PURPLE, ChildEdits::ALL),
    of::<GameModuleContent>("Game Module", "\u{e87b}", ORANGE, ChildEdits::NONE),
    of::<HotbarContent>("Hotbar", "\u{e1bd}", ORANGE, ChildEdits::NONE),
    of::<ImageContent>("Image", "\u{e3f4}", TEAL, ChildEdits::NONE),
    of::<InputRemapperContent>("Input Remaps", "\u{e312}", PURPLE, ChildEdits::NONE),
    of::<InputSettingsContent>("Input", "\u{e312}", GREEN, ChildEdits::NONE),
    of::<LogicGameContent>("Logic Game", "\u{ea28}", PURPLE, ChildEdits::NONE),
    of::<LogicGridContent>("Logic Grid", "\u{efa7}", GREEN, ChildEdits::NONE),
    of::<MapContent>("Map", "\u{e55b}", TEAL, ChildEdits::ALL),
    of::<PanZoomContent>("Pan and Zoom", "\u{f655}", GOLD, ChildEdits::NONE),
    of::<PdfContent>("PDF", "\u{e415}", BLUE, ChildEdits::NONE),
    of::<PixelArtContent>("Pixel Art", "\u{e3ec}", GREEN, ChildEdits::NONE),
    of::<PixelRayTracerContent>("Pixel Ray Tracer", "\u{e3e4}", PURPLE, ChildEdits::NONE),
    of::<PresentationContent>("Presentation", "\u{e41b}", ORANGE, ChildEdits::ALL),
    of::<RepositoryContent>("Repository", "\u{e889}", GOLD, ChildEdits::NONE),
    of::<Scene3dContent>("3D scene", "\u{efc9}", RED, ChildEdits::NONE),
    of::<SettingsContent>("Settings", "\u{e8b8}", TEAL, ChildEdits::NONE),
    of::<SettingsProfileContent>("Settings Profile", "\u{e8b8}", GOLD, ChildEdits::NONE),
    of::<TextContent>("Text", "\u{e873}", RED, ChildEdits::REPLACE),
    of::<TriangleContent>("Triangle", "\u{e86b}", TEAL, ChildEdits::NONE),
    of::<VideoContent>("Video Editor", "\u{e404}", GREEN, ChildEdits::ALL),
    of::<WorkspaceContent>("Workspace", "\u{ea0f}", ORANGE, ChildEdits::ALL),
];

pub fn block_type(content_type: Uuid) -> Option<&'static BlockType> {
    BLOCK_TYPES
        .iter()
        .find(|block_type| block_type.content_type == content_type)
}
