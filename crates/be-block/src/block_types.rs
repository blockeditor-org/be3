use uuid::Uuid;

use crate::{
    AudioContent, BeuiDemoContent, BlockContent, BrowserTabContent, CalendarContent, CanvasContent,
    ChecklistContent, CheckoutContent, CompiledLogicContent, CounterContent, DatabaseContent,
    DatabaseSchemaContent, DatabaseViewContent, DeterministicGameContent, DisplaySettingsContent,
    EditorSettingsContent, EditorViewContent, FolderContent, GameModuleContent, HotbarContent,
    ImageContent, InputRemapperContent, InputSettingsContent, LogicGameContent, LogicGridContent,
    MapContent, PanZoomContent, PdfContent, PixelArtContent, PixelRayTracerContent,
    PresentationContent, RepositoryContent, Scene3dContent, SettingsContent,
    SettingsProfileContent, TerminalContent, TextContent, TriangleContent, VideoContent,
    WorkspaceContent,
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
    of::<AudioContent>("Audio", "\u{e31a}", BLUE, ChildEdits::NONE),
    of::<BeuiDemoContent>("beui demo", "\u{e4fa}", GREEN, ChildEdits::NONE),
    of::<BrowserTabContent>("Web Browser Tab", "\u{e0e8}", PURPLE, ChildEdits::NONE),
    of::<CalendarContent>("Calendar", "\u{e2b9}", ORANGE, ChildEdits::NONE),
    of::<CanvasContent>("Canvas", "\u{e131}", GOLD, ChildEdits::REPLACE),
    of::<ChecklistContent>("Checklist", "\u{e1d0}", RED, ChildEdits::NONE),
    of::<CheckoutContent>("Checkout", "\u{e125}", BLUE, ChildEdits::NONE),
    of::<CompiledLogicContent>("Compiled Logic", "\u{e0a9}", RED, ChildEdits::NONE),
    of::<CounterContent>("Counter", "\u{e0ef}", TEAL, ChildEdits::NONE),
    of::<DatabaseContent>("Database", "\u{e0ad}", GOLD, ChildEdits::NONE),
    of::<DatabaseSchemaContent>("Database Schema", "\u{e4db}", BLUE, ChildEdits::NONE),
    of::<DatabaseViewContent>("Database View", "\u{e157}", GREEN, ChildEdits::NONE),
    of::<DeterministicGameContent>("Game", "\u{e0e9}", PURPLE, ChildEdits::NONE),
    of::<DisplaySettingsContent>("Display", "\u{e603}", BLUE, ChildEdits::NONE),
    of::<EditorSettingsContent>("Default editors", "\u{e603}", ORANGE, ChildEdits::NONE),
    of::<EditorViewContent>("View", "\u{e0ba}", TEAL, ChildEdits::NONE),
    of::<FolderContent>("Folder", "\u{e0d7}", PURPLE, ChildEdits::ALL),
    of::<GameModuleContent>("Game Module", "\u{e29c}", ORANGE, ChildEdits::NONE),
    of::<HotbarContent>("Hotbar", "\u{e4fa}", ORANGE, ChildEdits::NONE),
    of::<ImageContent>("Image", "\u{e0f6}", TEAL, ChildEdits::NONE),
    of::<InputRemapperContent>("Input Remaps", "\u{e284}", PURPLE, ChildEdits::NONE),
    of::<InputSettingsContent>("Input", "\u{e284}", GREEN, ChildEdits::NONE),
    of::<LogicGameContent>("Logic Game", "\u{e0df}", PURPLE, ChildEdits::NONE),
    of::<LogicGridContent>("Logic Grid", "\u{e425}", GREEN, ChildEdits::NONE),
    of::<MapContent>("Map", "\u{e110}", TEAL, ChildEdits::ALL),
    of::<PanZoomContent>("Pan and Zoom", "\u{e121}", GOLD, ChildEdits::NONE),
    of::<PdfContent>("PDF", "\u{e329}", BLUE, ChildEdits::NONE),
    of::<PixelArtContent>("Pixel Art", "\u{e0e9}", GREEN, ChildEdits::NONE),
    of::<PixelRayTracerContent>("Pixel Ray Tracer", "\u{e47e}", PURPLE, ChildEdits::NONE),
    of::<PresentationContent>("Presentation", "\u{e4ae}", ORANGE, ChildEdits::ALL),
    of::<RepositoryContent>("Repository", "\u{e40a}", GOLD, ChildEdits::NONE),
    of::<Scene3dContent>("3D scene", "\u{e061}", RED, ChildEdits::NONE),
    of::<SettingsContent>("Settings", "\u{e154}", TEAL, ChildEdits::NONE),
    of::<SettingsProfileContent>("Settings Profile", "\u{e154}", GOLD, ChildEdits::NONE),
    of::<TerminalContent>("Terminal", "\u{e181}", GREEN, ChildEdits::NONE),
    of::<TextContent>("Text", "\u{e0cc}", RED, ChildEdits::REPLACE),
    of::<TriangleContent>("Triangle", "\u{e192}", TEAL, ChildEdits::NONE),
    of::<VideoContent>("Video Editor", "\u{e0d0}", GREEN, ChildEdits::ALL),
    of::<WorkspaceContent>("Workspace", "\u{e2d0}", ORANGE, ChildEdits::ALL),
];

pub fn block_type(content_type: Uuid) -> Option<&'static BlockType> {
    BLOCK_TYPES
        .iter()
        .find(|block_type| block_type.content_type == content_type)
}
