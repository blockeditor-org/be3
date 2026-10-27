use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::list::List;
use crate::row::{RowFace, RowSize, RowTrailing};
use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor};
use crate::text_input::text_input_style;
use crate::theme::use_theme;
use crate::tokens::{Padding, Size, Space, use_space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    CommandPaletteApi, CommandRowHandle, PaletteItem, PaletteItems, PaletteScore, TextInputHandle,
};
use beui_core::base::TextAlign;
use beui_core::icons::ICON_SEARCH;
use beui_core::node::NodeId;
use beui_view::reactive::*;

const WIDTH: f32 = 560.0;
const LIST_HEIGHT: f32 = 360.0;
const MARGIN: Space = Space::Lg;
const PADDING: Space = Space::Sm;
const SEARCH_HEIGHT: Size = Size::Custom(38.0);
const PADDING_HORIZONTAL: Space = Space::Md;
const ROW_SPACING: Space = Space::Lg;

#[component]
pub fn CommandPalette(
    #[prop(default = ComponentRef::new())] api: ComponentRef<CommandPaletteApi>,
    open: Binding<bool>,
    #[prop(default = PaletteItems::Actions)] items: PaletteItems,
    #[prop(default = PaletteItem::substring_score as PaletteScore)] score: PaletteScore,
    #[prop(default = create_signal(String::new()))] query: Signal<String>,
    #[prop(default = "Run a command".to_owned())] placeholder: Prop<String>,
    #[prop(default = "Command palette".to_owned())] label: Prop<String>,
    #[prop(default = "command-palette".to_owned())] id: String,
    #[prop(default = None)] shown_in: Prop<Option<beui_core::window::WindowId>>,
    on_choose: Callback<String>,
) -> NodeId {
    let theme = use_theme();
    let scrim = create_memo(move || ColorRole::ScrimSoft.resolve(&theme));
    let search_id = format!("{id}.search");
    let row_id = id.clone();
    view! {
        <unstyled::CommandPalette
            api
            open
            items
            score
            query
            label
            shown_in
            on_choose={move |key: String| on_choose.call(key)}
            list_height=LIST_HEIGHT
            spacing={use_space(PADDING).get_untracked()}
            scrim={scrim}
            search_placeholder={placeholder}
            search_content={move |handle| view! {
                <PaletteSearch @test_id={search_id.clone()} handle />
            }}
            search_style={text_input_style()}
            scrollbar={scrollbar_style()}
            row={move |handle: CommandRowHandle| {
                let row_test_id = format!("{row_id}.item.{}", handle.key);
                view! {
                    <PaletteRow @test_id={row_test_id} handle />
                }
            }}
        >|content|
            <PalettePanel @test_id={id.clone()}>{content}</PalettePanel>
        </unstyled::CommandPalette>
    }
}

#[component]
fn PalettePanel(children: Child) -> NodeId {
    view! {
        <Frame padding=MARGIN>
            <Frame
                width={Extent::at_most(WIDTH)}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Lg
                padding=PADDING
            >
                {children}
            </Frame>
        </Frame>
    }
}

#[component]
fn PaletteSearch(handle: TextInputHandle) -> NodeId {
    let TextInputHandle { field, .. } = handle;
    view! {
        <Frame
            height=SEARCH_HEIGHT
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Accent)}
            radius=Radius::Md
            padding={Padding::horizontal(PADDING_HORIZONTAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_SEARCH />
                </Text>
                <Frame @sizing=ItemSize::Percent(100.0)>{field}</Frame>
            </List>
        </Frame>
    }
}

#[component]
fn PaletteRow(handle: CommandRowHandle) -> NodeId {
    let CommandRowHandle {
        label,
        detail,
        glyph,
        image,
        shortcut,
        checked,
        disabled,
        highlighted,
        hovered,
        ..
    } = handle;
    let trailing = create_memo(move || match checked.get() {
        Some(true) => RowTrailing::Check,
        _ => RowTrailing::None,
    });
    view! {
        <RowFace
            hovered
            selected={highlighted}
            disabled
            size=RowSize::Large
            label
            detail
            glyph
            image
            leading=true
            shortcut
            trailing
        />
    }
}
