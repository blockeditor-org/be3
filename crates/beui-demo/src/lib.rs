use base_pages::{
    ControlFlowPage, FramesPage, InteractionPage, LayeringPage, ListsPage, TextNodesPage,
};
use beui::datetime::{Date, DateTime, HourCycle, Time};
use beui::icons::{
    ICON_ACCOUNT_TREE, ICON_ALT_ROUTE, ICON_BUG_REPORT, ICON_CALENDAR_MONTH, ICON_CODE,
    ICON_CONTENT_COPY, ICON_CONTENT_PASTE, ICON_CONTRAST, ICON_CROP_SQUARE, ICON_DASHBOARD,
    ICON_DELETE, ICON_DRAG_INDICATOR, ICON_DRAW, ICON_EDIT, ICON_FLIP_TO_FRONT, ICON_GRID_VIEW,
    ICON_INPUT, ICON_LAYERS, ICON_LINK, ICON_LIST, ICON_MAIL, ICON_MENU, ICON_MOUSE, ICON_NOTES,
    ICON_OPEN_IN_NEW, ICON_PALETTE, ICON_PLAY_ARROW, ICON_RADIO_BUTTON_CHECKED, ICON_SHARE,
    ICON_SMART_BUTTON, ICON_STAR, ICON_SWAP_VERT, ICON_TEXT_FIELDS, ICON_TITLE, ICON_TOGGLE_ON,
    ICON_TOUCH_APP, ICON_TUNE, ICON_VIEW_COLUMN, ICON_VIEW_QUILT, ICON_WIDGETS,
};
use beui::reactive::{
    Align, Callback, Canvas, CanvasItem, CanvasView, Child, Children, ClickCallback, ForEach,
    Frame, Func, Justify, List, ListChild, Memo, Prop, ReadSignal, Selector, Show, Spacer,
    SpanStyle, Text, TextSpan, VirtualList, WriteSignal, batch, build, clone, create_memo,
    create_selector, create_signal, focus_ring, provide_context, use_context, view, with_document,
};
use beui::styled::DocumentTheme;
use beui::styled::theme::{CARD_RADIUS, FONT_SMALL, NARROW_WIDTH, RADIUS};
use beui::styled::{
    Accordion, ActionRow, Body, Bordered, Button, ButtonVariant, Calendar, Caption, Card, Checkbox,
    Chip, Code, ColorInput, ColorPicker, ColorWheel, ContextMenu, DateTimeField, Dialog, Display,
    Docking, FocusRing, Fullscreen, Heading, Icon, IconButton, IconButtonSize, IconSized, Launcher,
    LauncherItem, Link, ListRow, Listbox, MenuButton, ModalSheet, NumberInput, OklchColorWheel,
    Paragraph, Popover, Progress, RadioGroup, ResponsiveTabs, Scroll, Select, SelectableText,
    Separator, Shortcut, Slider, Spinner, SplitButton, Stack, Switch, Tabs, TextArea, TextInput,
    Theme, ThemeProvider, Title, Toast, Toasts, ToggleButton, Tooltip, Tree, TreeRowFace,
    use_theme,
};
use beui::unstyled::{
    ChoiceOption, Container, DateTimeParts, DockMode, DockPane, DockSplit, DockTab, DockingLayout,
    MAX_SCALE, MIN_SCALE, PanZoom, PanZoomHandle, PanZoomView, PopoverHandle, SliderScale,
    SyntaxColors, TabId, TextAreaState, TreeItem, dock_actions, narrower_than,
};
use beui::{
    Color32, Context, Direction, Document, FontId, Image, ItemSize, NodeId, Rect, TextAlign,
    unstyled,
};
use beui_macros::{component, sample};
use collaboration::CollaborationPage;
use sample::{Sample, ScrollPage};
use std::rc::Rc;
use std::sync::Arc;
use styled_pages::{
    ButtonsPage, CanvasPage, ChoicesPage, ColorsPage, DockingPage, EditorPage, InputsPage,
    LabelledSwitch, LayoutPage, MenusPage, OverlaysPage, PickersPage, RowsPage, TextPage,
    ThemesPage, TreePage,
};
use text_editor_core::{
    EditorCommand, Highlighter, Language, MarkdownCommand, TextBuffer, TextLanguage,
};
use unstyled_pages::{
    DraggingPage, PopupsPage, PressingPage, ScrollingPage, SelectingPage, ValuesPage,
};

mod base_pages;
mod collaboration;
mod sample;
mod styled_pages;
mod unstyled_pages;

const CATALOG: TabId = TabId::new(1);
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

const STYLED_PAGES: [Page; 16] = [
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

const PAGES: [Page; 28] = [
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
            Page::Text => "Type scale, icons, chips, links",
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
            Page::Collaboration => "Two editors over a simulated network",
            Page::Canvas => "A pannable, zoomable stage",
            Page::Themes => "The dark and e-ink themes",
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
                    <Container>
                        {move |_| view! {
                            <DemoShell />
                        }}
                    </Container>
                }
            }),
        }
    }
}

#[cfg(test)]
impl DemoApp {
    fn alone(today: Date, page: Page) -> Self {
        Self {
            document: build(move || {
                provide_context(Today(today));
                view! {
                    <Container>
                        {move |_| view! {
                            <LonePage page />
                        }}
                    </Container>
                }
            }),
        }
    }
}

#[cfg(test)]
#[component]
fn LonePage(page: Page) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
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

#[derive(Clone)]
struct Pages {
    layout: DockingLayout<TabId>,
    open: ReadSignal<Vec<Page>>,
    set_open: WriteSignal<Vec<Page>>,
}

impl Pages {
    fn new() -> Self {
        let (open, set_open) = create_signal(vec![Page::Docking]);
        Self {
            layout: DockingLayout::new(),
            open,
            set_open,
        }
    }

    fn open(&self, page: Page) {
        match self.open.with_untracked(|open| open.contains(&page)) {
            true => self.layout.show(&page.tab()),
            false => self.set_open.update(|open| open.push(page)),
        }
    }

    fn close(&self, page: Page) {
        self.set_open
            .update(|open| open.retain(|other| *other != page));
    }

    fn reset(&self) {
        batch(|| {
            self.set_open.set(vec![Page::Docking]);
            self.layout.reset();
        });
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
#[component]
fn DemoShell() -> NodeId {
    let theme = use_theme();
    let pages = Pages::new();
    let (mobile, set_mobile) = create_signal(false);
    let narrow = narrower_than(NARROW_WIDTH);
    let mode = create_memo(clone!(mobile narrow -> move || {
        match mobile.get() || narrow.get() {
            true => DockMode::Stacked,
            false => DockMode::Tiled,
        }
    }));
    let active = create_selector(clone!(pages -> move || pages.active()));
    let layout = pages.layout.clone();
    let open = pages.open.clone();
    let catalog = pages.clone();
    let toolbar = pages.clone();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Frame padding_horizontal=SHELL_PADDING padding_vertical=SHELL_PADDING>
                    <DemoToolbar pages=toolbar mobile set_mobile />
                </Frame>
                <Separator />
                <Docking @sizing=ItemSize::Percent(100.0) layout mode home=CATALOG>
                    <DockSplit id="shell" fraction={1.0 - PAGE_SHARE}>
                        <DockPane id="catalog">
                            <DockTab id=CATALOG title="Components" icon=ICON_WIDGETS>
                                <CatalogPanel pages={catalog.clone()} active={active.clone()} />
                            </DockTab>
                        </DockPane>
                        <DockPane
                            id="pages"
                            empty={move || view! {
                                <EmptyPanel />
                            }}
                        >
                            <ForEach keys={open}>
                                {move |page: Page| clone!(pages -> view! {
                                    <DockTab
                                        id={page.tab()}
                                        title={page.title()}
                                        icon={page.icon()}
                                        on_close={move || pages.close(page)}
                                    >
                                        <Container>
                                            {move |_| view! {
                                                <PageView page />
                                            }}
                                        </Container>
                                    </DockTab>
                                })}
                            </ForEach>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </List>
        </Frame>
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
fn DemoToolbar(pages: Pages, mobile: ReadSignal<bool>, set_mobile: WriteSignal<bool>) -> NodeId {
    let (eink, set_eink) = create_signal(false);
    let narrow = narrower_than(NARROW_WIDTH);
    let wide = create_memo(clone!(narrow -> move || !narrow.get()));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <Title content="beui" />
            <Show condition={wide.clone()}>
                <Caption content="retained mode ui" />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Show condition={wide}>
                {move || clone!(set_mobile -> view! {
                    <Checkbox
                        label="Mobile"
                        checked={mobile.clone()}
                        on_change={move |on: bool| set_mobile.set(on)}
                    />
                })}
            </Show>
            <LabelledSwitch
                label="E-ink"
                on={eink}
                on_change={move |on: bool| {
                    set_eink.set(on);
                    let theme = match on {
                        true => Theme::EINK,
                        false => Theme::DARK,
                    };
                    with_document(|document| document.set_theme(theme));
                }}
            />
            <IconButton
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
    let unstyled_pages = pages.clone();
    let base_pages = pages.clone();
    view! {
        <Scroll>
            <Frame padding_horizontal=CATALOG_PADDING padding_vertical=CATALOG_PADDING>
                <List spacing=0.0>
                    <CatalogGroup
                        title="Styled"
                        summary="Components painted with the theme"
                        listed={STYLED_PAGES.to_vec()}
                        pages
                        active={active.clone()}
                    />
                    <CatalogGroup
                        title="Unstyled"
                        summary="Behaviour only: you paint them"
                        listed={UNSTYLED_PAGES.to_vec()}
                        pages=unstyled_pages
                        active={active.clone()}
                    />
                    <CatalogGroup
                        title="Base"
                        summary="The nodes everything is built from"
                        listed={BASE_PAGES.to_vec()}
                        pages=base_pages
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
            <Frame padding_horizontal=CATALOG_GROUP_PADDING padding_vertical=CATALOG_GROUP_PADDING>
                <List spacing=2.0>
                    <Heading content={title} />
                    <Caption content={summary} ellipsis=true />
                </List>
            </Frame>
            <ForEach keys={listed}>
                {move |page: Page| {
                    let pages = pages.clone();
                    let selected = active.memo(Some(page));
                    view! {
                        <CatalogRow page pages selected />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn CatalogRow(page: Page, pages: Pages, selected: Memo<bool>) -> NodeId {
    view! {
        <ListRow
            @test_id={format!("demo.catalog.{}", page.title())}
            selected
            on_click={move || pages.open(page)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=CATALOG_SPACING>
                <Icon glyph={page.icon()} />
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <Body content={page.title()} />
                    <Caption content={page.summary()} ellipsis=true />
                </List>
            </List>
        </ListRow>
    }
}

#[component]
fn EmptyPanel() -> NodeId {
    view! {
        <Frame padding_horizontal=PAGE_PADDING padding_vertical=PAGE_PADDING>
            <List spacing=4.0 align=Align::Center>
                <Heading content="Nothing open" />
                <Caption content="Open a page from Components to get started." />
            </List>
        </Frame>
    }
}

#[cfg(test)]
mod tests;
