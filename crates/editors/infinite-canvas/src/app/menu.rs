use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_DOWNLOAD, ICON_EYE, ICON_GRID_3X3, ICON_LAYERS, ICON_PEN_TOOL, ICON_PENCIL, ICON_PLUS,
    ICON_SLIDERS_HORIZONTAL,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::actions::CanvasActions;
use super::state::Tool;
use beui::ItemSize;

const TOOLS: [Tool; 11] = [
    Tool::Select,
    Tool::Lasso,
    Tool::Frame,
    Tool::Line,
    Tool::Arrow,
    Tool::Rectangle,
    Tool::Ellipse,
    Tool::Diamond,
    Tool::Text,
    Tool::Pen,
    Tool::Eraser,
];

fn act(action: &Action) -> MenuNode {
    MenuNode::action(action)
}

fn add_menu(actions: &CanvasActions) -> MenuNode {
    MenuNode::submenu(
        "canvas.add",
        "Add",
        ICON_PLUS,
        vec![
            MenuNode::labelled(&actions.add_rectangle, "Rectangle"),
            MenuNode::labelled(&actions.add_line, "Line"),
            MenuNode::labelled(&actions.add_text, "Text"),
            MenuNode::labelled(&actions.add_image, "Image…"),
            MenuNode::labelled(&actions.add_block, "Block…"),
        ],
    )
}

fn export_menu(actions: &CanvasActions) -> MenuNode {
    MenuNode::submenu(
        "canvas.export",
        "Export",
        ICON_DOWNLOAD,
        vec![
            act(&actions.copy_png),
            act(&actions.copy_svg),
            MenuNode::Separator,
            act(&actions.export_png),
            act(&actions.export_svg),
        ],
    )
}

fn arrange_menu(actions: &CanvasActions) -> MenuNode {
    MenuNode::submenu(
        "canvas.arrange",
        "Arrange",
        ICON_LAYERS,
        vec![
            act(&actions.group),
            act(&actions.ungroup),
            MenuNode::Separator,
            act(&actions.lock),
            act(&actions.unlock),
            MenuNode::Separator,
            act(&actions.front),
            act(&actions.forward),
            act(&actions.backward),
            act(&actions.back),
        ],
    )
}

fn tools_menu(actions: &CanvasActions) -> MenuNode {
    MenuNode::submenu(
        "canvas.tools",
        "Tools",
        ICON_PEN_TOOL,
        TOOLS
            .into_iter()
            .map(|tool| act(&actions.tool(tool)))
            .chain([MenuNode::Separator])
            .chain(actions.pixel.iter().map(act))
            .chain([MenuNode::Separator])
            .chain(actions.raster.all().into_iter().map(act))
            .chain([
                MenuNode::Separator,
                act(&actions.tool_lock),
                act(&actions.bind_lines),
            ])
            .collect(),
    )
}

fn grid_menu(actions: &CanvasActions) -> MenuNode {
    let grid = &actions.grid;
    MenuNode::submenu(
        "canvas.grid-menu",
        "Grid",
        ICON_GRID_3X3,
        vec![
            act(&grid.shown),
            MenuNode::Separator,
            MenuNode::labelled(&grid.small, "Small"),
            MenuNode::labelled(&grid.medium, "Medium"),
            MenuNode::labelled(&grid.large, "Large"),
        ],
    )
}

fn profile_menu(actions: &CanvasActions) -> MenuNode {
    MenuNode::submenu(
        "canvas.profile",
        "Profile",
        ICON_SLIDERS_HORIZONTAL,
        vec![
            MenuNode::labelled(&actions.prototyping, "Prototyping"),
            MenuNode::labelled(&actions.vector, "Vector"),
            MenuNode::labelled(&actions.raster_profile, "Raster"),
        ],
    )
}

fn view_menu(actions: &CanvasActions) -> MenuNode {
    MenuNode::submenu(
        "canvas.view",
        "View",
        ICON_EYE,
        vec![
            grid_menu(actions),
            act(&actions.ignore_touch),
            MenuNode::Separator,
            act(&actions.fit_selection),
            act(&actions.fit_frame),
            act(&actions.fit_all),
        ],
    )
}

pub(crate) fn frame_menu(actions: &CanvasActions) -> Vec<MenuNode> {
    vec![
        MenuNode::submenu(
            "canvas.edit-menu",
            "Edit",
            ICON_PENCIL,
            vec![
                act(&actions.cut),
                act(&actions.copy),
                act(&actions.paste),
                act(&actions.duplicate),
                act(&actions.delete),
                MenuNode::Separator,
                act(&actions.select_all),
                act(&actions.invert),
            ],
        ),
        add_menu(actions),
        export_menu(actions),
        arrange_menu(actions),
        tools_menu(actions),
        view_menu(actions),
        profile_menu(actions),
    ]
}

fn context_menu(actions: &CanvasActions) -> Vec<MenuNode> {
    vec![
        add_menu(actions),
        MenuNode::Separator,
        act(&actions.edit),
        act(&actions.toggle_mode),
        act(&actions.fit_selection),
        MenuNode::Separator,
        act(&actions.cut),
        act(&actions.copy),
        act(&actions.paste),
        act(&actions.duplicate),
        act(&actions.delete),
        export_menu(actions),
        MenuNode::Separator,
        arrange_menu(actions),
        MenuNode::Separator,
        act(&actions.select_all),
        act(&actions.invert),
        MenuNode::Separator,
        tools_menu(actions),
        grid_menu(actions),
        profile_menu(actions),
    ]
}

#[component]
pub(crate) fn CanvasMenu(disabled: bool, children: Child) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let nodes = context_menu(&actions);
    view! {
        <ContextMenu
            items={view! {
                <MenuNodes nodes test_prefix={"infinite-canvas.menu".to_owned()} />
            }}
            child_size=ItemSize::Percent(100.0)
            disabled={disabled}
        >
            {children}
        </ContextMenu>
    }
}
