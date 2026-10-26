pub mod emoji;
pub mod find;
pub mod format_bar;

use crate::frame::{Border, Frame, Radius};
use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::button::{Button, ButtonSize};
use crate::checkbox::Checkbox;
use crate::context_menu::menu_style;
use crate::frame::ColorRole;
use crate::scroll::scrollbar_style;
use crate::text::{Text, TextColor, TextSize};
use crate::theme::{Theme, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    BlockGeometry, ChipGeometry, Completer, MarkerColumn, MarkerKind, Prose, QuoteBar,
    RemoteTextCursor, SyntaxColors, TableFill, TableLines, TextAreaColors, TextAreaGeometry,
    TextAreaLook, TextAreaState, TextLeaf, TextLine, TextMarker, TextWidget, WidgetGeometry,
};
use beui_view::primitives::{self, Layers, Span};
use beui_view::reactive::NodeId;
use beui_view::types::Color32;
use beui_view::types::PickedFile;
use beui_view::types::icons::ICON_COPY;
use beui_view::types::{Align, Direction, ItemSize};
use beui_view::types::{Rect, Vec2};

use crate::list::List;
use crate::tokens::{CornerSet, Padding, Size, Space, Stroke, resolved};
use beui_view::reactive::*;
use emoji::emoji_menu;

use beui_view::types::{Align2, Sides, SpanBackground};
use find::FindBar;

pub fn text_area_colors(theme: &Theme) -> TextAreaColors {
    TextAreaColors {
        gutter_border: theme.border,
        gutter_text: theme.text_muted,
        gutter_arrow: theme.text_muted,
        code_background: theme.surface_raised,
        widget: theme.pressed,
        broken_widget: theme.danger_soft,
        selection: theme.accent_soft,
        caret: theme.accent,
        placeholder: theme.text_muted,
        remote_selection_alpha: REMOTE_SELECTION_ALPHA,
        syntax: theme.syntax,
    }
}

pub fn text_area_geometry() -> TextAreaGeometry {
    TextAreaGeometry {
        caret_width: CARET_WIDTH,
        line_padding: LINE_PADDING,
        end_room: END_ROOM,
        gutter_padding: (GUTTER_PADDING, GUTTER_PADDING),
        gutter_arrow: TextSize::Base.px(),
        ellipsis_gap: ELLIPSIS_GAP,
        run_chip: ChipGeometry {
            height: RUN_CHIP_HEIGHT,
            padding: RUN_CHIP_PADDING,
            radius: RUN_CHIP_RADIUS,
            font_size: TextSize::Xs.px(),
        },
        widget: WidgetGeometry {
            height: WIDGET_HEIGHT,
            radius: WIDGET_RADIUS,
            icon_width: WIDGET_ICON_WIDTH,
            icon_size: TextSize::Lg.px(),
        },
        blocks: BlockGeometry {
            heading_space: HEADING_SPACE,
            quote_space: QUOTE_SPACE,
            code_space: CODE_SPACE,
            table_space: TABLE_SPACE,
            rule_gap: RULE_GAP,
            cell_padding: CELL_PADDING,
            delimiter_size: DELIMITER_SIZE,
        },
    }
}

pub fn text_area_look() -> TextAreaLook {
    TextAreaLook {
        leaf: RenderFn::new(|leaf: TextLeaf| {
            view! {
                <DocumentLeaf leaf />
            }
        }),
        marker: RenderFn::new(|marker: TextMarker| {
            view! {
                <DocumentMarker marker />
            }
        }),
        line: RenderFn::new(|line: TextLine| {
            view! {
                <DocumentLine line />
            }
        }),
        geometry: Prop::Static(text_area_geometry()),
    }
}

#[component]
pub fn TextArea(
    #[prop(default = ComponentRef::new())] api: ComponentRef<TextAreaSurfaceApi>,
    state: TextAreaState,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = TextColor::Default)] color: Prop<TextColor>,
    #[prop(default = TextSize::Base)] size: Prop<TextSize>,
    #[prop(default = TEXT_PADDING)] padding: Prop<Padding>,
    #[prop(default = None)] prose: Prop<Option<Prose>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] read_only: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = true)] line_numbers: bool,
    #[prop(default = true)] find: bool,
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
    on_content_size: Callback<Vec2>,
    on_image_paste: Callback<PickedFile>,
) -> NodeId {
    let theme = use_theme();
    let colors = create_memo(move || {
        let colors = text_area_colors(&theme.get());
        match color.get() {
            TextColor::Default => colors,
            color => {
                let text = color.resolve(&theme);
                TextAreaColors {
                    placeholder: text.scale_alpha(PLACEHOLDER_ALPHA),
                    syntax: SyntaxColors::uniform(text),
                    ..colors
                }
            }
        }
    });
    let font_size = create_memo(move || size.get().px());
    let padding = resolved(padding, Padding::resolve);
    let disabled = create_memo(move || disabled.get());
    let read_only = create_memo(move || read_only.get());
    let surface = NodeRef::new();
    api.fill(TextAreaSurfaceApi(surface));
    let block = block.unwrap_or_else(blank_widget);
    let selected_widget = selected_widget.unwrap_or_else(blank_widget);
    let document = text_area_look();
    let look = TextAreaLook {
        leaf: leaf.unwrap_or(document.leaf),
        marker: marker.unwrap_or(document.marker),
        line: line.unwrap_or(document.line),
        ..document
    };
    view! {
        <List spacing=Space::None>
            <Show condition={find}>||
                <FindBar state={state} read_only />
            </Show>
            <unstyled::TextContextMenu
                @sizing=ItemSize::Percent(100.0)
                state={state}
                menu={Some(menu_style())}
                masked={password}
                disabled
                read_only
                child_size=ItemSize::Percent(100.0)
            >|open_menu: Callback<Option<Rect>>|
                <unstyled::TextArea
                    @node_ref=&surface
                    state={state}
                    widgets
                    colors
                    font_size
                    padding
                    prose
                    disabled
                    read_only
                    focused
                    line_numbers
                    find
                    on_content_size
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
                    look
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

const PLACEHOLDER_ALPHA: f32 = 0.4;
const TEXT_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Md);
const CARET_WIDTH: f32 = Stroke::Focus.px();
const LINE_PADDING: (f32, f32) = (3.0, Space::Xs.px());
const END_ROOM: f32 = Space::Xl.px();
const GUTTER_PADDING: f32 = 10.0;
const ELLIPSIS_GAP: f32 = Space::Sm.px();
const RUN_CHIP_HEIGHT: f32 = Size::Xs.px();
const RUN_CHIP_PADDING: f32 = Space::Xs.px();
const RUN_CHIP_RADIUS: f32 = 3.0;
const WIDGET_HEIGHT: f32 = Size::Md.px();
const WIDGET_RADIUS: f32 = 5.0;
const WIDGET_ICON_WIDTH: f32 = 26.0;
const HEADING_SPACE: [f32; 3] = [Space::Xl.px(), Space::Lg.px(), Space::Sm.px()];
const QUOTE_SPACE: f32 = 7.0;
const CODE_SPACE: f32 = 5.0;
const TABLE_SPACE: f32 = Space::Xs.px();
const RULE_GAP: f32 = Space::Md.px();
const CELL_PADDING: f32 = Space::Lg.px();
const DELIMITER_SIZE: f32 = 11.0;
const REMOTE_SELECTION_ALPHA: u8 = 70;
const RULE_WIDTH: f32 = Stroke::Hairline.px();
const QUOTE_BAR_WIDTH: f32 = 3.0;
const CODE_INSET: f32 = 3.0;
const CODE_RADIUS: Radius = Radius::Sm;
const CODE_BLOCK_RADIUS: Radius = Radius::Md;
const QUOTE_BAR_RADIUS: Radius = Radius::Xs;
const COPY_INSET: Space = Space::Xs;
const HOLLOW_BULLET_WIDTH: Stroke = Stroke::Custom(1.25);
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
        MarkerKind::Bullet if marker.written().get_untracked() => {
            let font = create_memo(move || style.get().font);
            let color = create_memo(move || style.get().color.scale_alpha(NUMBER_ALPHA));
            view! {
                <primitives::Text font color>{marker.label}</primitives::Text>
            }
        }
        MarkerKind::Bullet => {
            let size = marker.bullet();
            let side = create_memo(move || Size::Custom(size.get()));
            let color = create_memo(move || style.get().color.scale_alpha(NUMBER_ALPHA));
            let hollow = marker.hollow();
            let fill = create_memo(move || match hollow.get() {
                true => ColorRole::None,
                false => ColorRole::Custom(color.get()),
            });
            let ring = create_memo(move || {
                hollow
                    .get()
                    .then(|| Border::new(ColorRole::Custom(color.get())).width(HOLLOW_BULLET_WIDTH))
            });
            view! {
                <Frame width=side height=side fill={fill} border=ring radius=Radius::Full />
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
    let left = line.left();
    let column = line.column();
    let right = line.right();
    let code_padding = line.code_padding();
    let outset = create_memo(move || match code_padding.get() > 0.0 {
        true => 0.0,
        false => CODE_INSET.min(left.get()),
    });
    let outer = create_memo(move || {
        let outset = outset.get();
        Padding::custom(Sides::new(
            left.get() - outset,
            0.0,
            (right.get() - outset).max(0.0),
            0.0,
        ))
    });
    let inner = create_memo(move || {
        let (above, below) = line.space.get();
        let side = outset.get() + code_padding.get();
        Padding::custom(Sides::new(side, above, side, below))
    });
    let code = create_memo(move || match look.get().code_block {
        true => ColorRole::SurfaceRaised,
        false => ColorRole::None,
    });
    let corners = create_memo(move || {
        let look = look.get();
        CornerSet::new(
            !look.code_above,
            !look.code_above,
            !look.code_below,
            !look.code_below,
        )
    });
    let quotes = line.quotes();
    let quote = line.quote_indent();
    let width = create_memo(move || Size::Custom(quote.get()));
    let room =
        create_memo(move || Padding::ZERO.right(Space::Custom(quote.get() - QUOTE_BAR_WIDTH)));
    let bars = create_memo(move || Padding::ZERO.left(Space::Custom(column.get())));
    let ruled = create_memo(move || {
        Padding::horizontal(Space::Custom(column.get())).right(Space::Custom(right.get()))
    });
    let rule = create_memo(move || match look.get().rule {
        true => ColorRole::Border,
        false => ColorRole::None,
    });
    let opens = line.opens_code();
    let language = line.code_language();
    let corner = create_memo(move || {
        let right = (right.get() - outset.get()).max(0.0) + COPY_INSET.px();
        Padding::custom(Sides::new(
            0.0,
            line.space.get().0 + COPY_INSET.px(),
            right,
            0.0,
        ))
    });
    let text = line.text;
    let theme = use_theme();
    let border = create_memo(move || theme.border.get());
    let header = create_memo(move || theme.surface.get());
    view! {
        <Layers>
            <TableFill line fill=header />
            <Frame padding=outer>
                <Frame fill=code radius=CODE_BLOCK_RADIUS corners padding=inner>{text}</Frame>
            </Frame>
            <TableLines line color=border rule=RULE_WIDTH />
            <Show condition={opens}>
                <Frame padding=corner align={Align2::new(Align::End, Align::Start)}>
                    <List direction=Direction::Horizontal spacing=COPY_INSET align=Align::Center>
                        <Text size=TextSize::Xs color=TextColor::Muted>{language}</Text>
                        <Button
                            icon_only=true
                            @test_id={format!("text.copy_code.{}", line.line.get_untracked())}
                            glyph={ICON_COPY.to_owned()}
                            label="Copy code"
                            variant=ButtonVariant::Ghost
                            size=ButtonSize::Small
                            capture_presses=true
                            press_focus=false
                            on_click={move || copy_text(line.code_text())}
                        />
                    </List>
                </Frame>
            </Show>
            <Frame padding=bars>
                <List direction=Direction::Horizontal spacing=Space::None>
                    <ForEach keys={quotes}>|bar: QuoteBar|
                        <Frame width padding=room>
                            <Frame
                                fill=ColorRole::Accent
                                radius=QUOTE_BAR_RADIUS
                                corners={CornerSet::new(bar.opens, bar.opens, bar.closes, bar.closes)}
                            />
                        </Frame>
                    </ForEach>
                </List>
            </Frame>
            <Frame padding=ruled align={Align2::new(Align::Stretch, Align::Center)}>
                <Frame height={Size::Custom(RULE_WIDTH)} fill=rule />
            </Frame>
        </Layers>
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
