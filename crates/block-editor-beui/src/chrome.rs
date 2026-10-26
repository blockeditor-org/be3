use beui::icons::ICON_SLIDERS_HORIZONTAL;
use beui::reactive::*;
pub use beui::styled::theme::NARROW_WIDTH;
use beui::styled::*;
use beui::{NodeId, Vec2};

pub const SIDEBAR_WIDTH: Size = Size::Custom(260.0);

const PADDING: Space = Space::Lg;
const SPACING: Space = Space::Md;
const BAND_PADDING: Padding = Padding::symmetric(Space::Lg, Space::Md);
const BAND_SPACING: Space = Space::Md;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Left,
    #[default]
    Right,
}

#[derive(Clone)]
struct ChromeLayout {
    narrow: Memo<bool>,
    panels: Signal<Vec<NodeId>>,
    open: Signal<bool>,
    toolbars: Signal<usize>,
}

impl ChromeLayout {
    fn offers_sheet(&self) -> Memo<bool> {
        let narrow = self.narrow;
        let panels = self.panels;
        create_memo(move || narrow.get() && !panels.get().is_empty())
    }
}

pub fn narrow_chrome() -> Memo<bool> {
    match use_context::<ChromeLayout>() {
        Some(layout) => layout.narrow,
        None => create_memo(|| false),
    }
}

pub fn sheet_control() -> Option<Signal<bool>> {
    use_context::<ChromeLayout>().map(|layout| layout.open)
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
pub(crate) fn ChromeRoot(
    #[prop(default = false)] phone: Prop<bool>,
    #[prop(children)] content: Render<()>,
) -> NodeId {
    let phone = create_memo(move || phone.get());
    view! {
        <Container>|size: ReadSignal<Vec2>| {
            let narrow = narrower_than(NARROW_WIDTH);
            let panels = create_signal(Vec::<NodeId>::new());
            let open = create_signal(false);
            let toolbars = create_signal(0usize);
            let layout = ChromeLayout {
                narrow,
                panels,
                open,
                toolbars,
            };
            provide_context(layout.clone());
            let content = content.call(());
            let offers = layout.offers_sheet();
            let sheet = create_memo(move || offers.get() && open.get());
            let bare = create_memo(move || {
                offers.get() && toolbars.get() == 0 && !phone.get()
            });
            let listed = create_memo(move || offers.get() && phone.get());
            let opener = layout.open;
            let extent = create_memo(move || size.get().y);
            let close = layout.open;
            view! {
                <List spacing=Space::None>
                    <Show condition={bare}>
                        <Frame fill=ColorRole::Surface>
                            <List spacing=Space::None>
                                <Frame padding=BAND_PADDING>
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=Space::None
                                    >
                                        <SheetToggle />
                                    </List>
                                </Frame>
                                <Separator />
                            </List>
                        </Frame>
                    </Show>
                    <Show condition={listed}>
                        <PanelItem open={opener} />
                    </Show>
                    {content} @sizing=ItemSize::Percent(100.0)
                    <Show condition={sheet}>
                        <Sheet
                            extent={extent}
                            open={sheet}
                            rest={SHEET_STOPS[0]}
                            on_close={move || close.set(false)}
                        >
                            <List spacing=Space::None>
                                <ForEach keys={panels}>|panel: NodeId|
                                    <Portal @sizing=ItemSize::Percent(100.0) node={Some(panel)} />
                                </ForEach>
                            </List>
                        </Sheet>
                    </Show>
                </List>
            }
        }</Container>
    }
}

#[component]
fn PanelItem(open: Signal<bool>) -> NodeId {
    let opener = Action::new("chrome.open-panel", "Open the side panel", move || {
        open.set(true)
    })
    .glyph(ICON_SLIDERS_HORIZONTAL)
    .register();
    add_to_menu(vec![MenuNode::action(&opener)]);
    view! {
        <List spacing=Space::None />
    }
}

#[component]
fn SheetToggle() -> NodeId {
    let open = match use_context::<ChromeLayout>() {
        Some(layout) => layout.open,
        None => create_signal(false),
    };
    view! {
        <ToggleButton
            label="Sidebar"
            glyph=ICON_SLIDERS_HORIZONTAL
            icon_only=true
            pressed={open}
            @test_id={"chrome.sidebar"}
        />
    }
}

#[component]
pub fn Sidebar(
    #[prop(default = Side::Right)] side: Side,
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = SIDEBAR_WIDTH)] width: Prop<Size>,
    #[prop(children)] children: Render<(), Children<ListChild>>,
) -> DynamicSegment<ListChild> {
    let shown = create_memo(move || shown.get());
    view! {
        <SidebarFrame side shown={shown} width>
            <Frame padding=PADDING>
                <List spacing=SPACING>
                    <ShowKeepAlive condition={shown}>{children.call(())}</ShowKeepAlive>
                </List>
            </Frame>
        </SidebarFrame>
    }
}

#[component]
fn SidebarFrame(
    side: Side,
    shown: Prop<bool>,
    width: Prop<Size>,
    children: Child,
) -> DynamicSegment<ListChild> {
    let layout = use_context::<ChromeLayout>();
    let narrow = narrow_chrome();
    let shown = create_memo(move || shown.get());
    let docked = create_memo(move || shown.get() && !narrow.get());
    let folded = create_memo(move || shown.get() && narrow.get());
    if let Some(layout) = layout {
        fold_into_sheet(layout, children, folded);
    }
    let held = create_memo(move || docked.get().then_some(children));
    view! {
        <ShowKeepAlive condition={docked}>
            <Show condition={side == Side::Right}>
                <Separator direction=Direction::Vertical />
            </Show>
            <Frame width=width fill=ColorRole::Surface>
                <Scroll>
                    <Portal node={held} />
                </Scroll>
            </Frame>
            <Show condition={side == Side::Left}>
                <Separator direction=Direction::Vertical />
            </Show>
        </ShowKeepAlive>
    }
}

fn fold_into_sheet(layout: ChromeLayout, body: NodeId, folded: Memo<bool>) {
    let panels = layout.panels;
    create_effect(move || {
        let wanted = folded.get();
        let listed = panels.with_untracked(|panels| panels.contains(&body));
        if wanted == listed {
            return;
        }
        panels.update(|panels| match wanted {
            true => panels.push(body),
            false => panels.retain(|panel| *panel != body),
        });
    });
    let panels = layout.panels;
    on_cleanup(move || panels.update(|panels| panels.retain(|panel| *panel != body)));
}

#[component]
pub fn Toolbar(
    #[prop(default = true)] shown: Prop<bool>,
    #[prop(default = BAND_SPACING)] spacing: Prop<Space>,
    #[prop(default = false)] fit: Prop<bool>,
    #[prop(children)] children: Children<ListChild>,
) -> DynamicSegment<ListChild> {
    let row = NodeRef::new();
    arrow_focus(row, Direction::Horizontal);
    view! {
        <ToolbarFrame shown fit>
            <List
                @node_ref=&row
                direction=Direction::Horizontal
                align=Align::Center
                spacing={spacing}
                children={children}
            />
        </ToolbarFrame>
    }
}

#[component]
fn ToolbarFrame(shown: Prop<bool>, fit: Prop<bool>, children: Child) -> DynamicSegment<ListChild> {
    let shown = create_memo(move || shown.get());
    let fit = create_memo(move || fit.get());
    let squeezed = narrow_chrome();
    let narrow = create_memo(move || squeezed.get() && !fit.get());
    let roomy = create_memo(move || !narrow.get());
    let offers = match use_context::<ChromeLayout>() {
        Some(layout) => {
            count_toolbar(&layout, shown);
            let offers = layout.offers_sheet();
            create_memo(move || offers.get() && shown.get())
        }
        None => create_memo(|| false),
    };
    let fitted_toggle = create_memo(move || offers.get() && squeezed.get() && fit.get());
    let spread = create_memo(move || roomy.get().then_some(children));
    let scrolled = create_memo(move || narrow.get().then_some(children));
    view! {
        <ShowKeepAlive condition={shown}>
            <Frame fill=ColorRole::Surface>
                <List spacing=Space::None>
                    <Show condition={roomy}>
                        <Frame padding=BAND_PADDING>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::None
                            >
                                <Portal @sizing=ItemSize::Percent(100.0) node={spread} />
                                <Show condition={fitted_toggle}>
                                    <Frame padding={Padding::horizontal(Space::Lg)}>
                                        <SheetToggle />
                                    </Frame>
                                </Show>
                            </List>
                        </Frame>
                    </Show>
                    <Show condition={narrow}>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=Space::None
                        >
                            <Scroll
                                @sizing=ItemSize::Percent(100.0)
                                direction=Direction::Horizontal
                            >
                                <Frame padding=BAND_PADDING>
                                    <Portal node={scrolled} />
                                </Frame>
                            </Scroll>
                            <Show condition={offers}>
                                <Frame padding=BAND_PADDING>
                                    <SheetToggle />
                                </Frame>
                            </Show>
                        </List>
                    </Show>
                    <Separator />
                </List>
            </Frame>
        </ShowKeepAlive>
    }
}

fn count_toolbar(layout: &ChromeLayout, shown: Memo<bool>) {
    let toolbars = layout.toolbars;
    let counted = StoredCell::new(false);
    create_effect(move || {
        let now = shown.get();
        if counted.replace(now) != now {
            toolbars.update(|count| match now {
                true => *count += 1,
                false => *count -= 1,
            });
        }
    });
    on_cleanup(move || {
        if counted.get() {
            toolbars.update(|count| *count -= 1);
        }
    });
}
