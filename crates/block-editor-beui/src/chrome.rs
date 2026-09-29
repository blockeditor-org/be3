use beui::NodeId;
use beui::icons::ICON_TUNE;
use beui::reactive::{
    Align, Children, Direction, ForEach, Frame, ItemSize, List, ListChild, Memo, Portal, Prop,
    ReadSignal, Render, Show, WriteSignal, clone, component, component_size, create_effect,
    create_memo, create_signal, on_cleanup, provide_context, untrack, use_context, view,
};
use beui::styled::theme::BORDER_WIDTH;
pub use beui::styled::theme::NARROW_WIDTH;
use beui::styled::{SHEET_STOPS, Scroll, Separator, Sheet, ToggleButton, use_theme};

pub const SIDEBAR_WIDTH: f32 = 260.0;

const PADDING: f32 = 14.0;
const SPACING: f32 = 10.0;
const BAND_PADDING_HORIZONTAL: f32 = 12.0;
const BAND_PADDING_VERTICAL: f32 = 8.0;
const BAND_SPACING: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Left,
    #[default]
    Right,
}

#[derive(Clone)]
struct ChromeLayout {
    narrow: Memo<bool>,
    panels: ReadSignal<Vec<NodeId>>,
    set_panels: WriteSignal<Vec<NodeId>>,
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    set_toolbars: WriteSignal<usize>,
}

impl ChromeLayout {
    fn offers_sheet(&self) -> Memo<bool> {
        let narrow = self.narrow.clone();
        let panels = self.panels.clone();
        create_memo(move || narrow.get() && !panels.get().is_empty())
    }
}

pub fn narrow_chrome() -> Memo<bool> {
    match use_context::<ChromeLayout>() {
        Some(layout) => layout.narrow,
        None => create_memo(|| false),
    }
}

pub fn sheet_open() -> Memo<bool> {
    match use_context::<ChromeLayout>() {
        Some(layout) => {
            let offers = layout.offers_sheet();
            let open = layout.open;
            create_memo(move || offers.get() && open.get())
        }
        None => create_memo(|| false),
    }
}

#[component]
pub(crate) fn ChromeRoot(#[prop(children)] content: Render<()>) -> NodeId {
    let size = component_size();
    let narrow = create_memo(clone!(size -> move || {
        let width = size.get().x;
        width > 0.0 && width < NARROW_WIDTH
    }));
    let (panels, set_panels) = create_signal(Vec::<NodeId>::new());
    let (open, set_open) = create_signal(false);
    let (toolbars, set_toolbars) = create_signal(0usize);
    let layout = ChromeLayout {
        narrow,
        panels: panels.clone(),
        set_panels,
        open: open.clone(),
        set_open,
        set_toolbars,
    };
    provide_context(layout.clone());
    let content = content.call(());
    let offers = layout.offers_sheet();
    let sheet = create_memo(clone!(offers open -> move || offers.get() && open.get()));
    let bare = create_memo(clone!(offers toolbars -> move || offers.get() && toolbars.get() == 0));
    let theme = use_theme();
    let bar_color = theme.surface.clone();
    let extent = create_memo(clone!(size -> move || size.get().y));
    let close = layout.set_open.clone();
    view! {
        <List spacing=0.0>
            <Show condition={bare}>
                <Frame color={bar_color}>
                    <List spacing=0.0>
                        <Frame
                            padding_horizontal=BAND_PADDING_HORIZONTAL
                            padding_vertical=BAND_PADDING_VERTICAL
                        >
                            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                                <SheetToggle />
                            </List>
                        </Frame>
                        <Separator />
                    </List>
                </Frame>
            </Show>
            {content} @sizing=ItemSize::Percent(100.0)
            <Show condition={sheet.clone()}>
                <Sheet
                    extent={extent}
                    open={sheet}
                    rest={SHEET_STOPS[0]}
                    on_close={move || close.set(false)}
                >
                    <List spacing=0.0>
                        <ForEach keys={panels}>
                            {move |panel: NodeId| {
                                view! {
                                    <Portal @sizing=ItemSize::Percent(100.0) node={Some(panel)} />
                                }
                            }}
                        </ForEach>
                    </List>
                </Sheet>
            </Show>
        </List>
    }
}

#[component]
fn SheetToggle() -> NodeId {
    let Some(layout) = use_context::<ChromeLayout>() else {
        return view! {
            <List spacing=0.0 />
        };
    };
    let set_open = layout.set_open.clone();
    view! {
        <ToggleButton
            label="Sidebar"
            glyph={ICON_TUNE.to_owned()}
            icon_only=true
            pressed={layout.open.clone()}
            @test_id={"chrome.sidebar"}
            on_change={move |open: bool| set_open.set(open)}
        />
    }
}

#[component]
pub fn Sidebar(
    #[prop(default = Side::Right)] side: Side,
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = SIDEBAR_WIDTH)] width: Prop<f32>,
    #[prop(children)] children: Children<ListChild>,
) -> NodeId {
    let theme = use_theme();
    let layout = use_context::<ChromeLayout>();
    let narrow = narrow_chrome();
    let shown = create_memo(move || shown.get());
    let docked = create_memo(clone!(shown narrow -> move || shown.get() && !narrow.get()));
    let folded = create_memo(clone!(shown narrow -> move || shown.get() && narrow.get()));
    let leading = create_memo(clone!(docked -> move || docked.get() && side == Side::Right));
    let trailing = create_memo(clone!(docked -> move || docked.get() && side == Side::Left));
    let body = view! {
        <Scroll>
            <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                <List spacing=SPACING children={children} />
            </Frame>
        </Scroll>
    };
    if let Some(layout) = layout {
        fold_into_sheet(layout, body, folded);
    }
    let held = create_memo(clone!(docked -> move || docked.get().then_some(body)));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame visible={leading} width=BORDER_WIDTH color={theme.border.clone()} />
            <Frame visible={docked} width={width} color={theme.surface.clone()}>
                <List spacing=0.0>
                    <Portal @sizing=ItemSize::Percent(100.0) node={held} />
                </List>
            </Frame>
            <Frame visible={trailing} width=BORDER_WIDTH color={theme.border.clone()} />
        </List>
    }
}

fn fold_into_sheet(layout: ChromeLayout, body: NodeId, folded: Memo<bool>) {
    let panels = layout.panels.clone();
    let set_panels = layout.set_panels.clone();
    create_effect(move || {
        let wanted = folded.get();
        let listed = untrack(|| panels.get().contains(&body));
        if wanted == listed {
            return;
        }
        set_panels.update(|panels| match wanted {
            true => panels.push(body),
            false => panels.retain(|panel| *panel != body),
        });
    });
    let set_panels = layout.set_panels;
    on_cleanup(move || set_panels.update(|panels| panels.retain(|panel| *panel != body)));
}

#[component]
pub fn Toolbar(
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = BAND_SPACING)] spacing: Prop<f32>,
    #[prop(default = false)] fit: Prop<bool>,
    #[prop(children)] children: Children<ListChild>,
) -> NodeId {
    let shown = create_memo(move || shown.get());
    let theme = use_theme();
    let fit = create_memo(move || fit.get());
    let squeezed = narrow_chrome();
    let narrow = create_memo(clone!(squeezed fit -> move || squeezed.get() && !fit.get()));
    let roomy = create_memo(clone!(narrow -> move || !narrow.get()));
    let offers = match use_context::<ChromeLayout>() {
        Some(layout) => {
            count_toolbar(&layout, shown.clone());
            layout.offers_sheet()
        }
        None => create_memo(|| false),
    };
    let fitted_toggle = create_memo(clone!(offers squeezed fit -> move || {
        offers.get() && squeezed.get() && fit.get()
    }));
    let row = view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing={spacing}
            children={children}
        />
    };
    let spread = create_memo(clone!(roomy -> move || roomy.get().then_some(row)));
    let scrolled = create_memo(clone!(narrow -> move || narrow.get().then_some(row)));
    view! {
        <Frame visible={shown} color={theme.surface.clone()}>
            <List spacing=0.0>
                <Show condition={roomy}>
                    <Frame
                        padding_horizontal=BAND_PADDING_HORIZONTAL
                        padding_vertical=BAND_PADDING_VERTICAL
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                            <Portal @sizing=ItemSize::Percent(100.0) node={spread} />
                            <Show condition={fitted_toggle}>
                                <Frame padding_horizontal=BAND_PADDING_HORIZONTAL>
                                    <SheetToggle />
                                </Frame>
                            </Show>
                        </List>
                    </Frame>
                </Show>
                <Show condition={narrow}>
                    <Frame padding_vertical=BAND_PADDING_VERTICAL>
                        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                            <Scroll
                                @sizing=ItemSize::Percent(100.0)
                                direction=Direction::Horizontal
                            >
                                <Frame padding_horizontal=BAND_PADDING_HORIZONTAL>
                                    <Portal node={scrolled} />
                                </Frame>
                            </Scroll>
                            <Show condition={offers}>
                                <Frame padding_horizontal=BAND_PADDING_HORIZONTAL>
                                    <SheetToggle />
                                </Frame>
                            </Show>
                        </List>
                    </Frame>
                </Show>
                <Separator />
            </List>
        </Frame>
    }
}

fn count_toolbar(layout: &ChromeLayout, shown: Memo<bool>) {
    let set_toolbars = layout.set_toolbars.clone();
    let counted = std::rc::Rc::new(std::cell::Cell::new(false));
    create_effect(clone!(counted set_toolbars -> move || {
        let now = shown.get();
        if counted.replace(now) != now {
            set_toolbars.update(|count| match now {
                true => *count += 1,
                false => *count -= 1,
            });
        }
    }));
    on_cleanup(move || {
        if counted.get() {
            set_toolbars.update(|count| *count -= 1);
        }
    });
}
