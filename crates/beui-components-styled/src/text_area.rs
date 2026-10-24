pub mod emoji;
pub mod find;
pub mod format_bar;

use crate::frame::{Frame, Radius};
use beui_macros::{component, view};

use crate::checkbox::Checkbox;
use crate::context_menu::menu_style;
use crate::frame::ColorRole;
use crate::scroll::scrollbar_style;
use crate::text::TextSize;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    Completer, MarkerColumn, MarkerKind, QUOTE_BAR_WIDTH, RemoteTextCursor, TextAreaColors,
    TextAreaState, TextLeaf, TextLine, TextMarker, TextWidget,
};
use beui_core::base::{Direction, ItemSize};
use beui_core::color::Color32;
use beui_core::file_picker::PickedFile;
use beui_core::geometry::{Rect, Vec2};
use beui_core::node::NodeId;
use beui_view::primitives::{self, *};
use beui_view::reactive::*;
use emoji::emoji_menu;

use find::FindBar;

#[component]
pub fn TextArea(
    #[prop(default = ComponentRef::new())] api: ComponentRef<TextAreaSurfaceApi>,
    state: TextAreaState,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] show_runs: Prop<bool>,
    #[prop(default = Vec::new())] client_colors: Prop<Vec<(u64, Color32)>>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    leaf: Option<RenderFn<TextLeaf, Span>>,
    marker: Option<RenderFn<TextMarker>>,
    line: Option<RenderFn<TextLine>>,
    #[prop(default = true)] emoji: bool,
    #[prop(default = false)] pastes_images: bool,
    on_widget_press: Callback<usize, bool>,
    on_focus_change: Callback<bool>,
    on_image_paste: Callback<PickedFile>,
) -> NodeId {
    let theme = use_theme();
    let colors = create_memo(move || TextAreaColors {
        gutter_border: theme.border.get(),
        gutter_text: theme.text_muted.get(),
        gutter_arrow: theme.text_muted.get(),
        code_background: theme.surface_raised.get(),
        widget: theme.pressed.get(),
        broken_widget: theme.danger_soft.get(),
        selection: theme.accent_soft.get(),
        caret: theme.accent.get(),
        placeholder: theme.text_muted.get(),
        syntax: theme.syntax.get(),
    });
    let surface = NodeRef::new();
    api.fill(TextAreaSurfaceApi(surface));
    let block = block.unwrap_or_else(blank_widget);
    let selected_widget = selected_widget.unwrap_or_else(blank_widget);
    let leaf = leaf.unwrap_or_else(|| {
        RenderFn::new(|leaf: TextLeaf| {
            view! {
                <DocumentLeaf leaf />
            }
        })
    });
    let marker = marker.unwrap_or_else(|| {
        RenderFn::new(|marker: TextMarker| {
            view! {
                <DocumentMarker marker />
            }
        })
    });
    let line = line.unwrap_or_else(|| {
        RenderFn::new(|line: TextLine| {
            view! {
                <DocumentLine line />
            }
        })
    });
    view! {
        <List spacing=0.0>
            <FindBar state={state} />
            <unstyled::TextContextMenu
                @sizing=ItemSize::Percent(100.0)
                state={state}
                menu={menu_style()}
                masked={password}
                child_size=ItemSize::Percent(100.0)
            >|open_menu: Callback<Option<Rect>>|
                <unstyled::TextArea
                    @node_ref=&surface
                    state={state}
                    widgets
                    colors
                    remote_cursors
                    drop_caret
                    placeholder
                    password
                    show_runs
                    client_colors
                    scrollbar={scrollbar_style()}
                    on_widget_press={on_widget_press}
                    on_focus_change={on_focus_change}
                    pastes_images
                    on_image_paste={on_image_paste}
                    on_toolbar={move |at: Option<Rect>| open_menu.call(at)}
                    block={block}
                    selected_widget={selected_widget}
                    leaf
                    marker
                    line
                    completer={match emoji {
                        true => unstyled::emoji_completer(),
                        false => Completer::none(),
                    }}
                    completion_menu={emoji_menu()}
                />
            </unstyled::TextContextMenu>
        </List>
    }
}

const CODE_INSET: f32 = 3.0;
const CODE_RADIUS: Radius = Radius::Sm;
const NUMBER_ALPHA: f32 = 0.6;

#[component]
pub fn DocumentLeaf(leaf: TextLeaf) -> Span {
    let theme = use_theme();
    let background = match leaf.syntax.code {
        true => Prop::dynamic(move || {
            Some(SpanBackground {
                color: ColorRole::SurfaceRaised.resolve(&theme),
                radius: CODE_RADIUS.px(),
                outset: Vec2::new(CODE_INSET, -1.0),
            })
        }),
        false => Prop::Static(None),
    };
    view! {
        <unstyled::LeafSpan leaf background />
    }
}

#[component]
pub fn DocumentMarker(marker: TextMarker) -> NodeId {
    let style = marker.style;
    let content = match marker.kind.get_untracked() {
        MarkerKind::Bullet => {
            let size = marker.bullet();
            let side = create_memo(move || Extent::px(size.get()));
            let color = create_memo(move || style.get().color);
            view! {
                <Frame width=side height=side fill={ColorRole::custom(color)} radius=Radius::Full />
            }
        }
        MarkerKind::Number(_) => {
            let font = create_memo(move || style.get().font);
            let color = create_memo(move || style.get().color.scale_alpha(NUMBER_ALPHA));
            view! {
                <primitives::Text font color>{marker.number()}</primitives::Text>
            }
        }
        MarkerKind::Check(_) => {
            let TextMarker {
                checked,
                disabled,
                line,
                ..
            } = marker;
            let size = create_memo(move || TextSize::Custom(style.get().font.size));
            view! {
                <Checkbox
                    @test_id={format!("text.checkbox.{line}")}
                    checked
                    disabled
                    size
                    capture_presses=true
                    tab_stop=false
                    press_focus=false
                />
            }
        }
        MarkerKind::Void(_) => {
            let gap = marker.gap();
            return view! {
                <primitives::Frame padding=gap />
            };
        }
    };
    view! {
        <MarkerColumn marker content />
    }
}

#[component]
pub fn DocumentLine(line: TextLine) -> NodeId {
    let look = line.look;
    let outer = create_memo(move || Sides::horizontal((line.padding.get() - CODE_INSET).max(0.0)));
    let indent = line.indent();
    let inner = create_memo(move || {
        let inset = line.padding.get().min(CODE_INSET);
        Sides::new(inset + indent.get(), 0.0, inset, 0.0)
    });
    let code = create_memo(move || match look.get().code_block {
        true => ColorRole::SurfaceRaised,
        false => ColorRole::None,
    });
    let quotes = line.quotes();
    let quote = line.quote_indent();
    let width = create_memo(move || Extent::px(quote.get()));
    let room = create_memo(move || Sides::new(0.0, 0.0, quote.get() - QUOTE_BAR_WIDTH, 0.0));
    let text = line.text;
    view! {
        <Frame padding=outer>
            <Layers>
                <Frame fill=code padding=inner>{text}</Frame>
                <List direction=Direction::Horizontal spacing=0.0>
                    <ForEach keys={quotes}>|_: usize|
                        <Frame width padding=room>
                            <Frame fill=ColorRole::Accent radius=Radius::Full />
                        </Frame>
                    </ForEach>
                </List>
            </Layers>
        </Frame>
    }
}

fn blank_widget() -> RenderFn<usize> {
    RenderFn::new(|_| {
        view! {
            <Frame />
        }
    })
}

#[derive(Clone, Copy)]
pub struct TextAreaSurfaceApi(NodeRef);

impl TextAreaSurfaceApi {
    pub fn surface(&self) -> NodeId {
        self.0.get()
    }
}
