use base_pages::{
    ControlFlowPage, FramesPage, InteractionPage, LayeringPage, ListsPage, TextNodesPage,
};
use beui::KeyAt;
use beui::datetime::{Date, DateTime, HourCycle, Time};
use beui::icons::{
    ICON_ACCOUNT_TREE, ICON_ALT_ROUTE, ICON_BUG_REPORT, ICON_CALENDAR_MONTH, ICON_CODE,
    ICON_CONTENT_COPY, ICON_CONTENT_PASTE, ICON_CONTRAST, ICON_CROP_SQUARE, ICON_DASHBOARD,
    ICON_DELETE, ICON_DRAG_INDICATOR, ICON_DRAW, ICON_EDIT, ICON_FLIP_TO_FRONT, ICON_FORMAT_BOLD,
    ICON_GRID_VIEW, ICON_INPUT, ICON_LAYERS, ICON_LINK, ICON_LIST, ICON_LOCK, ICON_MAIL, ICON_MENU,
    ICON_MOUSE, ICON_NOTES, ICON_OPEN_IN_NEW, ICON_PALETTE, ICON_PLAY_ARROW,
    ICON_POWER_SETTINGS_NEW, ICON_RADIO_BUTTON_CHECKED, ICON_SHARE, ICON_SMART_BUTTON, ICON_STAR,
    ICON_SWAP_VERT, ICON_TEXT_FIELDS, ICON_TITLE, ICON_TOGGLE_ON, ICON_TOUCH_APP, ICON_TUNE,
    ICON_VIEW_COLUMN, ICON_VIEW_QUILT, ICON_VOLUME_DOWN, ICON_VOLUME_OFF, ICON_VOLUME_UP,
    ICON_WIDGETS,
};
use beui::reactive::*;
use beui::styled::theme::NARROW_WIDTH;
use beui::styled::*;
use beui::unstyled::{
    ChoiceOption, Container, DateTimeParts, DockDialog, DockDialogPresentation, DockMode, DockPane,
    DockSplit, DockTab, DockTabControl, DockingLayout, MAX_SCALE, MIN_SCALE, PanZoom,
    PanZoomHandle, PanZoomView, PopoverHandle, SliderScale, SyntaxColors, TabId, TextAreaState,
    TreeItem, dock_actions, narrower_than, use_dock_tab,
};
use beui::{
    Color32, Context, Direction, Document, FontId, Image, ItemSize, Key, Modifiers, NodeId, Rect,
    ScrollGesture, TextAlign, unstyled,
};
use beui_macros::{component, sample};
use collaboration::CollaborationPage;
use rich_text::RichTextPage;
use sample::{Sample, ScrollPage};
use std::rc::Rc;
use std::sync::Arc;
use styled_pages::{
    AttachedCard, AttachedDialog, ButtonsPage, CanvasPage, ChoicesPage, ColorsPage, DockingPage,
    EditorPage, InputsPage, LabelledSwitch, LayoutPage, MenusPage, OverlaysPage, PickersPage,
    RowsPage, TextPage, ThemesPage, TreePage,
};
use text_editor_core::{
    EditorCommand, Highlighter, Language, MarkdownCommand, TextBuffer, TextLanguage,
};
use unstyled_pages::{
    DraggingPage, PopupsPage, PressingPage, ScrollingPage, SelectingPage, ValuesPage,
};

mod base_pages;
mod collaboration;
mod rich_text;
mod sample;
mod styled_pages;
mod unstyled_pages;

const CATALOG: TabId = TabId::new(1);
const DIALOG_TABS: u64 = 1 << 40;
const DIALOG_SIZE: beui::Vec2 = beui::Vec2::new(460.0, 120.0);
const NESTED_DIALOG_INSET: beui::Vec2 = beui::Vec2::new(56.0, 16.0);
const CARD_SIZE: beui::Vec2 = beui::Vec2::new(440.0, 300.0);
const PAGE_SHARE: f32 = 0.76;
const SHELL_PADDING: f32 = 10.0;
const TOOLBAR_SPACING: f32 = 8.0;
const CATALOG_PADDING: f32 = 4.0;
const CATALOG_SPACING: f32 = 10.0;
const CATALOG_GROUP_PADDING: f32 = 12.0;
const PAGE_PADDING: f32 = 16.0;
const PAGE_SPACING: f32 = 32.0;
const SECTION_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 12.0;
const CODE_PADDING: f32 = 12.0;
const DARK_BACKGROUND: u32 = 384;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Page {
    Docking,
    Text,
    Buttons,
    Inputs,
    Choices,
    Pickers,
    Menus,
    Overlays,
    Rows,
    Tree,
    Layout,
    Editor,
    RichText,
    Collaboration,
    Canvas,
    Themes,
    Pressing,
    Values,
    Selecting,
    Popups,
    Dragging,
    Scrolling,
    Frames,
    TextNodes,
    Lists,
    ControlFlow,
    Interaction,
    Layering,
    Colors,
}

const STYLED_PAGES: [Page; 17] = [
    Page::Docking,
    Page::Text,
    Page::Buttons,
    Page::Inputs,
    Page::Choices,
    Page::Pickers,
    Page::Colors,
    Page::Menus,
    Page::Overlays,
    Page::Rows,
    Page::Tree,
    Page::Layout,
    Page::Editor,
    Page::RichText,
    Page::Collaboration,
    Page::Canvas,
    Page::Themes,
];

const UNSTYLED_PAGES: [Page; 6] = [
    Page::Pressing,
    Page::Values,
    Page::Selecting,
    Page::Popups,
    Page::Dragging,
    Page::Scrolling,
];

const BASE_PAGES: [Page; 6] = [
    Page::Frames,
    Page::TextNodes,
    Page::Lists,
    Page::ControlFlow,
    Page::Interaction,
    Page::Layering,
];

const PAGES: [Page; 29] = [
    Page::Docking,
    Page::Text,
    Page::Buttons,
    Page::Inputs,
    Page::Choices,
    Page::Pickers,
    Page::Colors,
    Page::Menus,
    Page::Overlays,
    Page::Rows,
    Page::Tree,
    Page::Layout,
    Page::Editor,
    Page::RichText,
    Page::Collaboration,
    Page::Canvas,
    Page::Themes,
    Page::Pressing,
    Page::Values,
    Page::Selecting,
    Page::Popups,
    Page::Dragging,
    Page::Scrolling,
    Page::Frames,
    Page::TextNodes,
    Page::Lists,
    Page::ControlFlow,
    Page::Interaction,
    Page::Layering,
];

impl Page {
    fn tab(self) -> TabId {
        TabId::new(self as u64 + 2)
    }

    fn of(tab: TabId) -> Option<Page> {
        PAGES.into_iter().find(|page| page.tab() == tab)
    }

    fn title(self) -> &'static str {
        match self {
            Page::Docking => "Docking",
            Page::Text => "Text",
            Page::Buttons => "Buttons",
            Page::Inputs => "Inputs",
            Page::Choices => "Choices",
            Page::Pickers => "Pickers",
            Page::Colors => "Colors",
            Page::Menus => "Menus",
            Page::Overlays => "Overlays",
            Page::Rows => "Rows",
            Page::Tree => "Tree",
            Page::Layout => "Layout",
            Page::Editor => "Editor",
            Page::RichText => "Rich text",
            Page::Collaboration => "Collaboration",
            Page::Canvas => "Canvas",
            Page::Themes => "Themes",
            Page::Pressing => "Pressing",
            Page::Values => "Values",
            Page::Selecting => "Selecting",
            Page::Popups => "Popups",
            Page::Dragging => "Dragging",
            Page::Scrolling => "Scrolling",
            Page::Frames => "Frames",
            Page::TextNodes => "Text nodes",
            Page::Lists => "Lists and grids",
            Page::ControlFlow => "Control flow",
            Page::Interaction => "Interaction",
            Page::Layering => "Layering",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Page::Docking => ICON_VIEW_QUILT,
            Page::Text => ICON_TEXT_FIELDS,
            Page::Buttons => ICON_SMART_BUTTON,
            Page::Inputs => ICON_TOGGLE_ON,
            Page::Choices => ICON_RADIO_BUTTON_CHECKED,
            Page::Pickers => ICON_CALENDAR_MONTH,
            Page::Colors => ICON_PALETTE,
            Page::Menus => ICON_MENU,
            Page::Overlays => ICON_LAYERS,
            Page::Rows => ICON_LIST,
            Page::Tree => ICON_ACCOUNT_TREE,
            Page::Layout => ICON_DASHBOARD,
            Page::Editor => ICON_NOTES,
            Page::RichText => ICON_FORMAT_BOLD,
            Page::Collaboration => ICON_SHARE,
            Page::Canvas => ICON_DRAW,
            Page::Themes => ICON_CONTRAST,
            Page::Pressing => ICON_TOUCH_APP,
            Page::Values => ICON_TUNE,
            Page::Selecting => ICON_INPUT,
            Page::Popups => ICON_OPEN_IN_NEW,
            Page::Dragging => ICON_DRAG_INDICATOR,
            Page::Scrolling => ICON_SWAP_VERT,
            Page::Frames => ICON_CROP_SQUARE,
            Page::TextNodes => ICON_TITLE,
            Page::Lists => ICON_VIEW_COLUMN,
            Page::ControlFlow => ICON_ALT_ROUTE,
            Page::Interaction => ICON_MOUSE,
            Page::Layering => ICON_FLIP_TO_FRONT,
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Page::Docking => "Tabs, splits, windows and groups",
            Page::Text => "Text styles, sizes, colour roles, icons in text, links",
            Page::Buttons => "Buttons, icon buttons, tooltips",
            Page::Inputs => "Fields, switches, sliders, progress",
            Page::Choices => "Radio groups, listboxes, tabs",
            Page::Pickers => "Dates and times",
            Page::Colors => "Color inputs, pickers and wheels",
            Page::Menus => "Context, menu and split buttons",
            Page::Overlays => "Popovers, dialogs, sheets",
            Page::Rows => "Virtual lists of thousands of rows",
            Page::Tree => "An expandable, keyboard-driven tree",
            Page::Layout => "Cards, accordions, stacks, scrolls",
            Page::Editor => "A markdown text area",
            Page::RichText => "Formatted text, alone and shared live",
            Page::Collaboration => "Two editors over a simulated network",
            Page::Canvas => "A pannable, zoomable stage",
            Page::Themes => "The built-in themes",
            Page::Pressing => "Pressables, buttons, toggles and rows",
            Page::Values => "Sliders, text and number inputs, colors",
            Page::Selecting => "Choices, disclosures, trees, text menus",
            Page::Popups => "Popovers, tooltips, menus, sheets, dates",
            Page::Dragging => "Draggables and drop targets",
            Page::Scrolling => "Scroll areas and scrollbars",
            Page::Frames => "Fills, outlines, padding, sizes",
            Page::TextNodes => "Styles, wrapping and spans",
            Page::Lists => "Direction, spacing, tracks",
            Page::ControlFlow => "Show, ForEach and Dynamic",
            Page::Interaction => "Hover, press, focus and keys",
            Page::Layering => "Layers and overlays",
        }
    }
}

pub struct DemoApp {
    pub document: Document,
}

#[derive(Clone, Copy)]
struct Today(Date);

impl Default for DemoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoApp {
    pub fn new() -> Self {
        Self::on(Date::today())
    }

    pub fn on(today: Date) -> Self {
        Self {
            document: build(move || {
                provide_context(Today(today));
                view! {
                    <Demo />
                }
            }),
        }
    }
}

#[component]
pub fn Demo() -> NodeId {
    view! {
        <Container>|_|
            <DemoShell />
        </Container>
    }
}

#[cfg(test)]
impl DemoApp {
    fn alone(today: Date, page: Page) -> Self {
        Self {
            document: build(move || {
                provide_context(Today(today));
                view! {
                    <Container>|_|
                        <LonePage page />
                    </Container>
                }
            }),
        }
    }
}

#[cfg(test)]
#[component]
fn LonePage(page: Page) -> NodeId {
    view! {
        <Frame fill=ColorRole::Background>
            <PageView page />
        </Frame>
    }
}

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}

#[derive(Clone, Copy)]
struct DragModifier {
    held: Signal<Option<Modifiers>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct DemoDialog {
    id: TabId,
    parent: TabId,
    modal: bool,
    card: bool,
    anchor: Option<[u32; 4]>,
    depth: u8,
}

impl DemoDialog {
    fn size(self) -> beui::Vec2 {
        match self.card {
            true => CARD_SIZE,
            false => DIALOG_SIZE - NESTED_DIALOG_INSET * f32::from(self.depth),
        }
    }

    fn anchor(self) -> Option<Rect> {
        self.anchor.map(|[left, top, right, bottom]| {
            Rect::from_min_max(
                beui::pos2(f32::from_bits(left), f32::from_bits(top)),
                beui::pos2(f32::from_bits(right), f32::from_bits(bottom)),
            )
        })
    }

    fn presentation(self) -> DockDialogPresentation {
        match self.card {
            true => DockDialogPresentation::Card,
            false => DockDialogPresentation::Window,
        }
    }

    fn title(self) -> &'static str {
        match (self.card, self.modal) {
            (true, _) => "Card",
            (false, true) => "Modal dialog",
            (false, false) => "Dialog",
        }
    }
}

#[derive(Clone, Copy)]
struct DemoDialogs {
    open: Signal<Vec<DemoDialog>>,
    next: Stored<std::cell::Cell<u64>>,
}

impl DemoDialogs {
    fn new() -> Self {
        let open = create_signal(Vec::new());
        Self {
            open,
            next: store(std::cell::Cell::new(DIALOG_TABS)),
        }
    }

    fn open(&self, parent: TabId, modal: bool) {
        self.add(parent, modal, false, None);
    }

    fn open_card(&self, parent: TabId, anchor: Option<Rect>) {
        self.add(parent, true, true, anchor);
    }

    fn add(&self, parent: TabId, modal: bool, card: bool, anchor: Option<Rect>) {
        let anchor =
            anchor.map(|rect| [rect.min.x, rect.min.y, rect.max.x, rect.max.y].map(f32::to_bits));
        let id = TabId::new(self.next.with(|next| next.replace(next.get() + 1)));
        self.open.update(|open| {
            let depth = open
                .iter()
                .find(|dialog| dialog.id == parent)
                .map_or(0, |dialog| dialog.depth + 1);
            open.push(DemoDialog {
                id,
                parent,
                modal,
                card,
                anchor,
                depth,
            });
        });
    }

    fn close(&self, id: TabId) {
        self.open.update(|open| {
            let mut closing = vec![id];
            while let Some(child) = open
                .iter()
                .find(|dialog| closing.contains(&dialog.parent) && !closing.contains(&dialog.id))
            {
                closing.push(child.id);
            }
            open.retain(|dialog| !closing.contains(&dialog.id));
        });
    }

    fn close_from(&self, parent: TabId) {
        let hanging: Vec<TabId> = self.open.with_untracked(|open| {
            open.iter()
                .filter(|dialog| dialog.parent == parent)
                .map(|dialog| dialog.id)
                .collect()
        });
        for id in hanging {
            self.close(id);
        }
    }
}

#[derive(Clone, Copy)]
struct Pages {
    layout: DockingLayout<TabId>,
    open: Signal<Vec<Page>>,
    dialogs: DemoDialogs,
}

impl Pages {
    fn new(dialogs: DemoDialogs) -> Self {
        let open = create_signal(vec![Page::Docking]);
        Self {
            layout: DockingLayout::new(),
            open,
            dialogs,
        }
    }

    fn open(&self, page: Page, beside: bool) {
        if self.open.with_untracked(|open| open.contains(&page)) {
            self.layout.show(&page.tab());
            return;
        }
        let shown = self.active().filter(|_| !beside);
        match shown {
            Some(shown) if self.layout.replace(&shown.tab(), page.tab()) => {
                self.dialogs.close_from(shown.tab());
                self.open.update(|open| {
                    for held in open.iter_mut().filter(|held| **held == shown) {
                        *held = page;
                    }
                });
            }
            _ => self.open.update(|open| open.push(page)),
        }
    }

    fn close(&self, page: Page) {
        self.dialogs.close_from(page.tab());
        self.open.update(|open| open.retain(|other| *other != page));
    }

    fn reset(&self) {
        self.open.set(vec![Page::Docking]);
        self.layout.reset();
    }

    fn active(&self) -> Option<Page> {
        self.layout.state().with(|state| {
            state
                .recent_tabs()
                .into_iter()
                .filter(|tab| *tab != CATALOG)
                .find(|tab| {
                    state
                        .find(*tab)
                        .is_some_and(|position| state.active_tab(position.leaf) == Some(*tab))
                })
                .and_then(Page::of)
        })
    }
}

#[sample]
fn switch_tabs_with_alt_q(layout: DockingLayout<TabId>) {
    let modifiers = held_modifiers();
    let next = Chord::key(Key::Q).alt();
    for (id, label, backwards, chord) in [
        (
            "demo.switch.next",
            "Switch to the tab shown before",
            false,
            next,
        ),
        (
            "demo.switch.previous",
            "Switch to the tab shown longest ago",
            true,
            next.shift(),
        ),
    ] {
        Action::new(id, label, move || {
            match layout.switching() {
                true => layout.step_switch(backwards),
                false => layout.begin_switch(backwards),
            }
            if !modifiers.get_untracked().alt {
                layout.commit_switch();
            }
        })
        .glyph(ICON_FLIP_TO_FRONT)
        .shortcut(chord)
        .at(KeyAt::Window)
        .register();
    }
    create_effect(move || {
        if !modifiers.get().alt && untrack(|| layout.switching()) {
            untrack(|| layout.commit_switch());
        }
    });
}

#[sample]
#[component]
fn DemoShell() -> NodeId {
    let dialogs = DemoDialogs::new();
    let pages = Pages::new(dialogs);
    let mobile = create_signal(false);
    let narrow = narrower_than(NARROW_WIDTH);
    let mode = create_memo(move || match mobile.get() || narrow.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    });
    let active = create_selector(move || pages.active());
    let layout = pages.layout;
    switch_tabs_with_alt_q(layout);
    let open = pages.open;
    let drag_modifier = create_signal(None);
    provide_context(DragModifier {
        held: drag_modifier,
    });
    provide_context(dialogs);
    let attached = dialogs.open;
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=0.0>
                <Frame padding=SHELL_PADDING>
                    <DemoToolbar pages mobile />
                </Frame>
                <Separator />
                <Docking @sizing=ItemSize::Percent(100.0) layout mode drag_modifier>
                    <DockSplit id="shell" fraction={1.0 - PAGE_SHARE}>
                        <DockPane id="catalog">
                            <DockTab id=CATALOG title="Components" icon=ICON_WIDGETS>
                                <CatalogPanel pages active={active} />
                            </DockTab>
                        </DockPane>
                        <DockPane
                            id="pages"
                            empty={move || view! {
                                <EmptyPanel />
                            }}
                        >
                            <ForEach keys={open}>|page: Page|
                                <DockTab
                                    id={page.tab()}
                                    title={page.title()}
                                    icon={page.icon()}
                                    on_close={move || pages.close(page)}
                                >
                                    <Container>|_|
                                        <PageView page />
                                    </Container>
                                </DockTab>
                            </ForEach>
                        </DockPane>
                    </DockSplit>
                    <ForEach keys={attached}>|dialog: DemoDialog|
                        <DockDialog
                            id={dialog.id}
                            parent={dialog.parent}
                            modal={dialog.modal}
                            presentation={dialog.presentation()}
                            anchor={dialog.anchor()}
                            dismiss_on_outside_press={dialog.card}
                            title={dialog.title()}
                            icon=ICON_OPEN_IN_NEW
                            size={dialog.size()}
                            on_close={move || dialogs.close(dialog.id)}
                        >
                            <DemoDialogBody dialog />
                        </DockDialog>
                    </ForEach>
                </Docking>
            </List>
        </Frame>
    }
}

#[component]
fn DemoDialogBody(dialog: DemoDialog) -> NodeId {
    match dialog.card {
        true => view! {
            <AttachedCard />
        },
        false => view! {
            <AttachedDialog modal={dialog.modal} />
        },
    }
}

#[component]
fn PageView(page: Page) -> NodeId {
    match page {
        Page::Docking => view! {
            <DockingPage />
        },
        Page::Text => view! {
            <TextPage />
        },
        Page::Buttons => view! {
            <ButtonsPage />
        },
        Page::Inputs => view! {
            <InputsPage />
        },
        Page::Choices => view! {
            <ChoicesPage />
        },
        Page::Pickers => view! {
            <PickersPage />
        },
        Page::Colors => view! {
            <ColorsPage />
        },
        Page::Menus => view! {
            <MenusPage />
        },
        Page::Overlays => view! {
            <OverlaysPage />
        },
        Page::Rows => view! {
            <RowsPage />
        },
        Page::Tree => view! {
            <TreePage />
        },
        Page::Layout => view! {
            <LayoutPage />
        },
        Page::Editor => view! {
            <EditorPage />
        },
        Page::RichText => view! {
            <RichTextPage />
        },
        Page::Collaboration => view! {
            <CollaborationPage />
        },
        Page::Canvas => view! {
            <CanvasPage />
        },
        Page::Themes => view! {
            <ThemesPage />
        },
        Page::Pressing => view! {
            <PressingPage />
        },
        Page::Values => view! {
            <ValuesPage />
        },
        Page::Selecting => view! {
            <SelectingPage />
        },
        Page::Popups => view! {
            <PopupsPage />
        },
        Page::Dragging => view! {
            <DraggingPage />
        },
        Page::Scrolling => view! {
            <ScrollingPage />
        },
        Page::Frames => view! {
            <FramesPage />
        },
        Page::TextNodes => view! {
            <TextNodesPage />
        },
        Page::Lists => view! {
            <ListsPage />
        },
        Page::ControlFlow => view! {
            <ControlFlowPage />
        },
        Page::Interaction => view! {
            <InteractionPage />
        },
        Page::Layering => view! {
            <LayeringPage />
        },
    }
}

#[component]
fn DemoToolbar(pages: Pages, mobile: Signal<bool>) -> NodeId {
    let selected_theme = create_signal(Some(0));
    let narrow = narrower_than(NARROW_WIDTH);
    let wide = create_memo(move || !narrow.get());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <Text style=TextStyle::TITLE string="beui" />
            <Show condition={wide}>
                <Text style=TextStyle::CAPTION string="retained mode ui" />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Show condition={wide}>
                <Checkbox label="Mobile" checked={mobile} />
            </Show>
            <Select
                label="Theme"
                options={view! {
                    <ForEach
                        keys={Theme::BUILT_IN.iter().map(|(label, _)| *label).collect::<Vec<_>>()}
                    >|label: &'static str|
                        <ChoiceOption label />
                    </ForEach>
                }}
                selected={Binding::new(selected_theme, move |index: Option<usize>| {
                    if let Some((_, theme)) = index.and_then(|index| Theme::BUILT_IN.get(index)) {
                        selected_theme.set(index);
                        with_document(|document| document.set_theme(*theme));
                    }
                })}
            />
            <IconButton
                @test_id={"demo.inspector"}
                glyph=ICON_BUG_REPORT
                label="Open the inspector"
                variant=ButtonVariant::Ghost
                on_click={|| with_document(Document::open_inspector)}
            />
            <Button
                label="Reset layout"
                variant=ButtonVariant::Secondary
                on_click={move || pages.reset()}
            />
        </List>
    }
}

#[component]
fn CatalogPanel(pages: Pages, active: Selector<Option<Page>>) -> NodeId {
    view! {
        <Scroll>
            <Frame padding=CATALOG_PADDING>
                <List spacing=0.0>
                    <CatalogGroup
                        title="Styled"
                        summary="Components painted with the theme"
                        listed={STYLED_PAGES.to_vec()}
                        pages
                        active={active}
                    />
                    <CatalogGroup
                        title="Unstyled"
                        summary="Behaviour only: you paint them"
                        listed={UNSTYLED_PAGES.to_vec()}
                        pages
                        active={active}
                    />
                    <CatalogGroup
                        title="Base"
                        summary="The nodes everything is built from"
                        listed={BASE_PAGES.to_vec()}
                        pages
                        active
                    />
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn CatalogGroup(
    title: &'static str,
    summary: &'static str,
    listed: Vec<Page>,
    pages: Pages,
    active: Selector<Option<Page>>,
) -> NodeId {
    view! {
        <List spacing=0.0>
            <Frame padding=CATALOG_GROUP_PADDING>
                <List spacing=2.0>
                    <Text style=TextStyle::HEADING string={title} />
                    <Text style=TextStyle::CAPTION string={summary} ellipsis=true />
                </List>
            </Frame>
            <ForEach keys={listed}>|page: Page| {
                let selected = active.memo(Some(page));
                view! {
                    <CatalogRow page pages selected />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn CatalogRow(page: Page, pages: Pages, selected: Memo<bool>) -> NodeId {
    let modifiers = held_modifiers();
    let open = move || {
        let held = modifiers.get_untracked();
        pages.open(page, held.ctrl || held.logo);
    };
    view! {
        <ListRow @test_id={format!("demo.catalog.{}", page.title())} selected on_click={open}>
            <List direction=Direction::Horizontal align=Align::Center spacing=CATALOG_SPACING>
                <Text align=TextAlign::Center>
                    <Icon glyph={page.icon()} />
                </Text>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <Text string={page.title()} />
                    <Text style=TextStyle::CAPTION string={page.summary()} ellipsis=true />
                </List>
            </List>
        </ListRow>
    }
}

#[component]
fn EmptyPanel() -> NodeId {
    view! {
        <Frame padding=PAGE_PADDING>
            <List spacing=4.0 align=Align::Center>
                <Text style=TextStyle::HEADING string="Nothing open" />
                <Text
                    style=TextStyle::CAPTION
                    string="Open a page from Components to get started."
                />
            </List>
        </Frame>
    }
}

#[cfg(test)]
mod tests;
