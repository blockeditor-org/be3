use beui::datetime::{Date, DateTime, HourCycle, Time};
use beui::icons::{
    ICON_ACCOUNT_TREE, ICON_BUG_REPORT, ICON_CALENDAR_MONTH, ICON_CONTENT_COPY, ICON_CONTRAST,
    ICON_DASHBOARD, ICON_DELETE, ICON_DRAW, ICON_EDIT, ICON_GRID_VIEW, ICON_LAYERS, ICON_LIST,
    ICON_MENU, ICON_NOTES, ICON_PALETTE, ICON_PLAY_ARROW, ICON_RADIO_BUTTON_CHECKED, ICON_SHARE,
    ICON_SMART_BUTTON, ICON_STAR, ICON_TEXT_FIELDS, ICON_TOGGLE_ON, ICON_VIEW_QUILT, ICON_WIDGETS,
};
use beui::reactive::{
    Align, Callback, Canvas, CanvasItem, CanvasView, Child, Children, ClickCallback, ForEach,
    Frame, Func, List, ListChild, Memo, Prop, ReadSignal, Selector, Show, Spacer, Text,
    VirtualList, WriteSignal, build, clone, create_memo, create_selector, create_signal,
    focus_ring, provide_context, use_context, view, with_document,
};
use beui::styled::DocumentTheme;
use beui::styled::theme::{CARD_RADIUS, NARROW_WIDTH, RADIUS};
use beui::styled::{
    Accordion, ActionRow, Body, Bordered, Button, ButtonVariant, Calendar, Caption, Card, Checkbox,
    Chip, Code, ColorInput, ColorPicker, ContextMenu, DateTimeField, Dialog, Display, DockArea,
    Fullscreen, Heading, Icon, IconButton, IconButtonSize, IconSized, Link, ListRow, Listbox,
    MenuButton, ModalSheet, NumberInput, Paragraph, Popover, Progress, RadioGroup, ResponsiveTabs,
    Scroll, Select, SelectableText, Separator, Shortcut, Slider, Spinner, SplitButton, Stack,
    Switch, Tabs, TextArea, TextInput, Theme, ThemeProvider, Title, ToggleButton, Tooltip, Tree,
    TreeRowFace, use_theme,
};
use beui::unstyled::{
    ChoiceOption, Container, DateTimeParts, DockMode, DockState, MAX_SCALE, MIN_SCALE, PanZoom,
    PanZoomHandle, PanZoomView, PopoverHandle, Side, SliderScale, TabId, TextAreaState, TreeItem,
    dock_actions, narrower_than,
};
use beui::{Color32, Context, Direction, Document, ItemSize, NodeId, Rect, TextAlign, unstyled};
use beui_macros::component;
use std::sync::Arc;
use text_editor_core::{EditorCommand, MarkdownCommand, TextBuffer, TextLanguage};

const CATALOG: TabId = TabId::new(1);
const PAGE_SHARE: f32 = 0.76;
const SHELL_PADDING: f32 = 10.0;
const TOOLBAR_SPACING: f32 = 8.0;
const CATALOG_PADDING: f32 = 4.0;
const CATALOG_SPACING: f32 = 10.0;
const PAGE_PADDING: f32 = 16.0;
const PAGE_SPACING: f32 = 24.0;
const SECTION_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 12.0;
const COLUMN_SPACING: f32 = 20.0;
const COLUMNS_BREAKPOINT: f32 = 460.0;
const TABS_NARROW_WIDTH: f32 = 380.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;
const TOOLBAR_RULE_LENGTH: f32 = 20.0;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_MIDPOINT: f32 = 1.0;
const TREE_HEIGHT: f32 = 240.0;
const NESTED_SCROLL_HEIGHT: f32 = 160.0;
const NESTED_SCROLL_ROWS: usize = 30;
const SWATCH_HEIGHT: f32 = 28.0;
const LARGE_ICON: f32 = 28.0;
const SPINNER_WIDTH: f32 = 120.0;
const DIALOG_WIDTH: f32 = 380.0;
const ROW_COUNT: usize = 10_000;
const ROW_HEIGHT: f32 = 34.0;
const COMPACT_ROW_HEIGHT: f32 = 25.0;
const ROW_PADDING_HORIZONTAL: f32 = 12.0;
const ROW_PADDING_VERTICAL: f32 = 9.0;
const COMPACT_ROW_PADDING_VERTICAL: f32 = 4.0;
const STRIP_COUNT: usize = 200;
const STRIP_HEIGHT: f32 = 52.0;
const STRIP_ITEM_WIDTH: f32 = 120.0;
const STAGE_VIEW: PanZoomView = PanZoomView::new(beui::pos2(200.0, 120.0), 0.8);
const STAGE_CARDS: [(f32, f32, f32, f32, &str); 4] = [
    (0.0, 0.0, 170.0, 90.0, "Inbox"),
    (230.0, 30.0, 150.0, 80.0, "Notes"),
    (50.0, 150.0, 210.0, 110.0, "Canvas"),
    (320.0, 190.0, 130.0, 70.0, "Archive"),
];
const STAGE_ZOOM_STEP: f32 = 1.3;
const STAGE_LABEL_SIZE: f32 = 15.0;
const STAGE_PADDING: f32 = 10.0;
const EDITOR_TEXT: &str = "# Notes\n\nA **multiline** editor with a gutter, wrapping and `markdown`\nstyling.\n\n- [x] click a checkbox\n- [ ] press Ctrl+F to find\n- [ ] type :smile to pick an emoji\n";
const TREE_NODES: [(&str, usize); 9] = [
    ("Project", 0),
    ("src", 1),
    ("main.rs", 2),
    ("demo.rs", 2),
    ("assets", 1),
    ("logo.png", 2),
    ("theme.toml", 2),
    ("README.md", 1),
    ("Cargo.toml", 1),
];
const FRUITS: [&str; 6] = ["Apple", "Banana", "Cherry", "Date", "Grape", "Mango"];

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
    Canvas,
    Themes,
}

const PAGES: [Page; 14] = [
    Page::Docking,
    Page::Text,
    Page::Buttons,
    Page::Inputs,
    Page::Choices,
    Page::Pickers,
    Page::Menus,
    Page::Overlays,
    Page::Rows,
    Page::Tree,
    Page::Layout,
    Page::Editor,
    Page::Canvas,
    Page::Themes,
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
            Page::Menus => "Menus",
            Page::Overlays => "Overlays",
            Page::Rows => "Rows",
            Page::Tree => "Tree",
            Page::Layout => "Layout",
            Page::Editor => "Editor",
            Page::Canvas => "Canvas",
            Page::Themes => "Themes",
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
            Page::Menus => ICON_MENU,
            Page::Overlays => ICON_LAYERS,
            Page::Rows => ICON_LIST,
            Page::Tree => ICON_ACCOUNT_TREE,
            Page::Layout => ICON_DASHBOARD,
            Page::Editor => ICON_NOTES,
            Page::Canvas => ICON_DRAW,
            Page::Themes => ICON_CONTRAST,
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Page::Docking => "Tabs, splits, windows and groups",
            Page::Text => "Type scale, icons, chips, links",
            Page::Buttons => "Buttons, icon buttons, tooltips",
            Page::Inputs => "Fields, switches, sliders, progress",
            Page::Choices => "Radio groups, listboxes, tabs",
            Page::Pickers => "Dates, times and colors",
            Page::Menus => "Context, menu and split buttons",
            Page::Overlays => "Popovers, dialogs, sheets",
            Page::Rows => "Virtual lists of thousands of rows",
            Page::Tree => "An expandable, keyboard-driven tree",
            Page::Layout => "Cards, accordions, stacks, scrolls",
            Page::Editor => "A markdown text area",
            Page::Canvas => "A pannable, zoomable stage",
            Page::Themes => "The dark and e-ink themes",
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

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}

fn starting_state() -> DockState {
    let mut state = DockState::new([CATALOG]);
    let catalog = state.leaves(state.main())[0];
    state.split(catalog, Side::Right, PAGE_SHARE, vec![Page::Docking.tab()]);
    state
}

fn settled(mut state: DockState) -> DockState {
    let open = state.all_tabs().into_iter().any(|tab| tab != CATALOG);
    if open {
        state.remove_empty_panes();
        return state;
    }
    let catalog = state.find(CATALOG).map(|position| position.leaf);
    let elsewhere = state
        .leaves(state.main())
        .into_iter()
        .any(|leaf| Some(leaf) != catalog);
    if let (false, Some(catalog)) = (elsewhere, catalog) {
        state.split(catalog, Side::Right, PAGE_SHARE, Vec::new());
    }
    state
}

fn open(state: &mut DockState, tab: TabId) {
    if state.contains(tab) {
        state.show(tab);
        return;
    }
    let catalog = state.find(CATALOG).map(|position| position.leaf);
    let elsewhere = state
        .surfaces()
        .into_iter()
        .flat_map(|surface| state.leaves(surface))
        .find(|leaf| Some(*leaf) != catalog);
    let target = state
        .focused_leaf()
        .filter(|leaf| Some(*leaf) != catalog)
        .or(elsewhere);
    match (target, catalog) {
        (Some(leaf), _) => state.push(leaf, tab),
        (None, Some(catalog)) => {
            state.split(catalog, Side::Right, PAGE_SHARE, vec![tab]);
        }
        (None, None) => state.push_to_focused(tab),
    }
    state.show(tab);
}

fn tab_title(tab: TabId) -> String {
    match Page::of(tab) {
        Some(page) => page.title().to_owned(),
        None => "Components".to_owned(),
    }
}

fn tab_icon(tab: TabId) -> String {
    match Page::of(tab) {
        Some(page) => page.icon().to_owned(),
        None => ICON_WIDGETS.to_owned(),
    }
}

#[component]
fn DemoShell() -> NodeId {
    let theme = use_theme();
    let (state, set_state) = create_signal(starting_state());
    let (mobile, set_mobile) = create_signal(false);
    let narrow = narrower_than(NARROW_WIDTH);
    let mode = create_memo(clone!(mobile narrow -> move || {
        match mobile.get() || narrow.get() {
            true => DockMode::Stacked,
            false => DockMode::Tiled,
        }
    }));
    let catalog_state = set_state.clone();
    let toolbar_state = set_state.clone();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Frame padding_horizontal=SHELL_PADDING padding_vertical=SHELL_PADDING>
                    <DemoToolbar set_state=toolbar_state mobile set_mobile />
                </Frame>
                <Separator />
                <DockArea
                    @sizing=ItemSize::Percent(100.0)
                    state={state}
                    mode={mode}
                    home={Some(CATALOG)}
                    title={Func::new(tab_title)}
                    icon={Func::new(tab_icon)}
                    closable={Func::new(|tab: TabId| tab != CATALOG)}
                    on_change={move |next: DockState| set_state.set(settled(next))}
                    on_close={move |_: TabId| {}}
                    empty={move || view! {
                        <EmptyPanel />
                    }}
                >
                    {move |tab: TabId| {
                        let set_state = catalog_state.clone();
                        match Page::of(tab) {
                            None => view! {
                                <CatalogPanel set_state />
                            },
                            Some(page) => view! {
                                <Container>
                                    {move |_| view! {
                                        <PagePanel page />
                                    }}
                                </Container>
                            },
                        }
                    }}
                </DockArea>
            </List>
        </Frame>
    }
}

#[component]
fn DemoToolbar(
    set_state: WriteSignal<DockState>,
    mobile: ReadSignal<bool>,
    set_mobile: WriteSignal<bool>,
) -> NodeId {
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
                on_click={move || set_state.set(settled(starting_state()))}
            />
        </List>
    }
}

#[component]
fn CatalogPanel(set_state: WriteSignal<DockState>) -> NodeId {
    view! {
        <Scroll>
            <Frame padding_horizontal=CATALOG_PADDING padding_vertical=CATALOG_PADDING>
                <List spacing=0.0>
                    <ForEach keys={PAGES.to_vec()}>
                        {move |page: Page| {
                            let set_state = set_state.clone();
                            view! {
                                <CatalogRow page set_state />
                            }
                        }}
                    </ForEach>
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn CatalogRow(page: Page, set_state: WriteSignal<DockState>) -> NodeId {
    view! {
        <ListRow
            @test_id={format!("demo.catalog.{}", page.title())}
            on_click={move || {
                set_state.update(|state| {
                    open(state, page.tab());
                    *state = settled(state.clone());
                });
            }}
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

#[component]
fn PagePanel(page: Page) -> NodeId {
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
        Page::Canvas => view! {
            <CanvasPage />
        },
        Page::Themes => view! {
            <ThemesPage />
        },
    }
}

#[component]
fn ScrollPage(children: Child) -> NodeId {
    view! {
        <Scroll>
            <Frame padding_horizontal=PAGE_PADDING padding_vertical=PAGE_PADDING>{children}</Frame>
        </Scroll>
    }
}

#[component]
fn FillPage(children: Child) -> NodeId {
    view! {
        <Frame padding_horizontal=PAGE_PADDING padding_vertical=PAGE_PADDING>{children}</Frame>
    }
}

#[component]
fn Section(title: &'static str, children: Children<ListChild>) -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <Heading content={title} />
            <List spacing=SECTION_SPACING children />
        </List>
    }
}

#[component]
fn LabelledSwitch(label: &'static str, on: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
            <Switch label on on_change={move |on: bool| on_change.call(on)} />
            <Body content={label} />
        </List>
    }
}

#[component]
fn DockingPage() -> NodeId {
    let (count, set_count) = create_signal(0u32);
    let counted = create_memo(clone!(count -> move || format!("{} edits", count.get())));
    let moved = create_memo(clone!(count -> move || count.get() > 0));
    let editing = set_count.clone();
    dock_actions(view! {
        <IconButton
            glyph=ICON_EDIT
            label="Edit"
            on_click={move || editing.update(|count| *count += 1)}
        />
    });
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Tabs">
                    <Paragraph
                        content="Drag a tab by its label. Drop it over the middle of a pane to join \
                         that pane, over an edge to split it, or over a tab bar to land between the \
                         tabs there. With a finger, pull the tab down out of its bar first: sliding \
                         along the bar scrolls it. The arrow keys walk a tab bar, and Ctrl+Tab and \
                         Ctrl+Shift+Tab walk the tabs of whichever pane you are in."
                    />
                </Section>
                <Section title="Splits">
                    <Paragraph
                        content="Drag the bar between two panes to resize them, or focus it with Tab \
                         and use the arrow keys. Drag the grip at the start of a pane's bar to move \
                         the whole pane, or right-click it to move the tabs into a sidebar and back."
                    />
                </Section>
                <Section title="Windows">
                    <Paragraph
                        content="Hold Alt while dragging a tab, or right-click one and pop it out, to \
                         float it in a window. A window moves by the grip at the left of its bar and \
                         resizes from any edge."
                    />
                </Section>
                <Section title="Groups">
                    <Paragraph
                        content="Drop a tab onto the middle of another tab to group them: the group \
                         becomes one tab, and choosing it shows a second tab bar holding the tabs \
                         inside. A group ungroups from its own right-click menu."
                    />
                </Section>
                <Section title="State follows the tab">
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Button
                            label="Edit"
                            variant=ButtonVariant::Secondary
                            on_click={move || set_count.update(|count| *count += 1)}
                        />
                        <Caption content={counted} />
                    </List>
                    <Show condition={moved}>
                        <Caption
                            content="Drag this tab somewhere else: the count comes with it."
                            wrap=true
                        />
                    </Show>
                    <Caption
                        content="Mobile, in the toolbar, stacks the panes as a phone would."
                        wrap=true
                    />
                </Section>
            </List>
        </ScrollPage>
    }
}

#[component]
fn TextPage() -> NodeId {
    let theme = use_theme();
    let (followed, set_followed) = create_signal("Nothing followed yet".to_owned());
    let set_guide = set_followed.clone();
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Type scale">
                    <Display content="Display" />
                    <Title content="Title" />
                    <Heading content="Heading" />
                    <Body content="Body text for labels and values." />
                    <Caption content="Caption text, muted, for secondary details." />
                    <Paragraph
                        content="A paragraph wraps to the width it is given, so it reads well \
                         in a narrow pane as well as a wide one."
                    />
                    <Code content="let answer = 6 * 7;" />
                </Section>
                <Section title="Icons and chips">
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Icon glyph=ICON_STAR />
                        <IconSized
                            glyph=ICON_PALETTE
                            font_size=LARGE_ICON
                            color={theme.accent.clone()}
                        />
                        <Chip label="Chip" />
                        <Chip label="Another chip" />
                    </List>
                </Section>
                <Section title="Shortcuts">
                    <Shortcut keys="Tab" description="move focus to the next control" />
                    <Shortcut keys="Shift+Tab" description="move focus back" />
                    <Shortcut keys="Enter" description="activate the focused control" />
                    <Shortcut keys="Ctrl+Z" description="undo an edit in a text field" />
                    <Shortcut keys="Ctrl+Shift+I" description="open the inspector" />
                    <Shortcut keys="Ctrl+Shift+C" description="pick a node to inspect" />
                </Section>
                <Section title="Links">
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=16.0
                        wrap=true
                    >
                        <Link
                            label="Open the grid"
                            glyph=ICON_GRID_VIEW
                            on_click={move || set_followed.set("Followed: Open the grid".to_owned())}
                        />
                        <Link
                            label="Read the guide"
                            on_click={move || set_guide.set("Followed: Read the guide".to_owned())}
                        />
                        <Link label="Unavailable" disabled=true />
                    </List>
                    <Caption content={followed} />
                </Section>
                <Section title="Selectable text">
                    <SelectableText child_size=ItemSize::Percent(100.0)>
                        <Paragraph
                            content="Drag across this paragraph to select it, or long-press a \
                             word with a finger. Right-click it for Copy and Select All."
                        />
                    </SelectableText>
                </Section>
            </List>
        </ScrollPage>
    }
}

#[component]
fn ButtonsPage() -> NodeId {
    let (clicked, set_clicked) = create_signal("Nothing clicked yet".to_owned());
    let (pinned, set_pinned) = create_signal("Selection is unpinned".to_owned());
    let primary = set_clicked.clone();
    let secondary = set_clicked.clone();
    let ghost = set_clicked.clone();
    let glyph = set_clicked.clone();
    let copy = set_clicked.clone();
    let share = set_clicked.clone();
    let remove = set_clicked.clone();
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Buttons">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0 wrap=true>
                        <Button
                            label="Primary"
                            variant=ButtonVariant::Primary
                            on_click={move || primary.set("Clicked Primary".to_owned())}
                        />
                        <Button
                            label="Secondary"
                            variant=ButtonVariant::Secondary
                            on_click={move || secondary.set("Clicked Secondary".to_owned())}
                        />
                        <Button
                            label="Ghost"
                            variant=ButtonVariant::Ghost
                            on_click={move || ghost.set("Clicked Ghost".to_owned())}
                        />
                        <Button
                            label="With an icon"
                            glyph=ICON_PLAY_ARROW
                            variant=ButtonVariant::Secondary
                            on_click={move || glyph.set("Clicked With an icon".to_owned())}
                        />
                        <Button label="Disabled" variant=ButtonVariant::Primary disabled=true />
                    </List>
                </Section>
                <Section title="Icon buttons">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0 wrap=true>
                        <IconButton
                            glyph=ICON_CONTENT_COPY
                            label="Copy"
                            variant=ButtonVariant::Primary
                            on_click={move || copy.set("Clicked Copy".to_owned())}
                        />
                        <IconButton
                            glyph=ICON_SHARE
                            label="Share"
                            on_click={move || share.set("Clicked Share".to_owned())}
                        />
                        <IconButton
                            glyph=ICON_DELETE
                            label="Delete"
                            variant=ButtonVariant::Ghost
                            size=IconButtonSize::Compact
                            on_click={move || remove.set("Clicked Delete".to_owned())}
                        />
                        <IconButton glyph=ICON_EDIT label="Edit" disabled=true />
                    </List>
                    <Caption content="Hover an icon button to see its label as a tooltip." />
                    <Caption content={clicked} />
                </Section>
                <Section title="Toggle button">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <ToggleButton
                            label="Pin selection"
                            pressed=false
                            on_change={move |pressed| {
                                let text = match pressed {
                                    true => "Selection is pinned",
                                    false => "Selection is unpinned",
                                };
                                set_pinned.set(text.to_owned());
                            }}
                        />
                    </List>
                    <Caption content={pinned} />
                </Section>
                <Section title="Tooltips">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Tooltip label="Tooltips wrap anything that can be hovered or focused">
                            <Chip label="Hover me" />
                        </Tooltip>
                    </List>
                </Section>
            </List>
        </ScrollPage>
    }
}

#[component]
fn InputsPage() -> NodeId {
    let (greeting, set_greeting) = create_signal(greeting_label(""));
    let (seats, set_seats) = create_signal(4.0f64);
    let seats_text = create_memo(clone!(seats -> move || format!("{} seats", seats.get())));
    let (load, set_load) = create_signal(0.4f32);
    let load_text = create_memo(clone!(load -> move || percent_label(load.get())));
    let (zoom, set_zoom) = create_signal(1.0f32);
    let zoom_text = create_memo(clone!(zoom -> move || format!("{:.2}x", zoom.get())));
    let (locked, set_locked) = create_signal(true);
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Text input">
                    <TextInput
                        value=String::new()
                        placeholder="Type a name"
                        on_change={move |value: String| set_greeting.set(greeting_label(&value))}
                    />
                    <Caption content={greeting} />
                    <Caption
                        content="Click to place the caret, drag to select, and Ctrl+Z to undo."
                        wrap=true
                    />
                </Section>
                <Section title="Number input">
                    <NumberInput
                        value={seats}
                        min=1.0
                        max=12.0
                        label="Seats"
                        on_change={move |value| set_seats.set(value)}
                    />
                    <Caption content={seats_text} />
                </Section>
                <Section title="Sliders and progress">
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Caption content="Simulated load" />
                        <Caption
                            @sizing=ItemSize::Percent(100.0)
                            content={load_text}
                            align=TextAlign::End
                        />
                    </List>
                    <Slider value=0.4 label="Load" on_change={move |value| set_load.set(value)} />
                    <Progress value={load} />
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Caption content="Zoom, with 1x in the middle" />
                        <Caption
                            @sizing=ItemSize::Percent(100.0)
                            content={zoom_text}
                            align=TextAlign::End
                        />
                    </List>
                    <Slider
                        value={zoom}
                        min=ZOOM_MIN
                        max=ZOOM_MAX
                        scale={SliderScale::Midpoint(ZOOM_MIDPOINT)}
                        label="Zoom"
                        on_change={move |value| set_zoom.set(value)}
                    />
                    <Caption content="Something that takes as long as it takes" />
                    <Spinner width=SPINNER_WIDTH label="Working" />
                </Section>
                <Section title="Disabled controls">
                    <LabelledSwitch
                        label="Locked"
                        on={locked.clone()}
                        on_change={move |on| set_locked.set(on)}
                    />
                    <TextInput value="Read only while locked" disabled={locked.clone()} />
                    <Select
                        options={view! {
                            <ChoiceOption label="String" />
                            <ChoiceOption label="Number" />
                        }}
                        selected=Some(0)
                        label="Field type"
                        disabled={locked.clone()}
                    />
                    <Checkbox label="Minimum" checked=false disabled={locked.clone()} />
                    <Button label="Save" variant=ButtonVariant::Primary disabled={locked} />
                </Section>
            </List>
        </ScrollPage>
    }
}

fn greeting_label(name: &str) -> String {
    match name.is_empty() {
        true => "Nobody yet".to_owned(),
        false => format!("Hello, {name}"),
    }
}

fn percent_label(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

#[component]
fn ChoicesPage() -> NodeId {
    let modes = ["Automatic", "Manual", "Scheduled"];
    let (mode_text, set_mode_text) = create_signal("Automatic updates".to_owned());
    let colors = ["Amber", "Blue", "Green", "Purple"];
    let (color_text, set_color_text) = create_signal("Blue selected".to_owned());
    let (fruit_text, set_fruit_text) = create_signal("Apple selected".to_owned());
    let (scale, set_scale) = create_signal(1usize);
    let scale_text = create_memo(clone!(scale -> move || format!("Drawn at {}x", scale.get() + 1)));
    let (view_tab, set_view_tab) = create_signal(0usize);
    let view_text = create_memo(clone!(view_tab -> move || {
        format!("Showing the {} view", ["day", "week", "month", "year"][view_tab.get()])
    }));
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                    <Section @sizing=ItemSize::Percent(50.0) title="Radio group">
                        <RadioGroup
                            options={view! {
                                <ChoiceOption label="Automatic" />
                                <ChoiceOption label="Manual" />
                                <ChoiceOption label="Scheduled" />
                            }}
                            selected=Some(0)
                            on_change={move |selected: Option<usize>| {
                                if let Some(index) = selected {
                                    set_mode_text.set(format!("{} updates", modes[index]));
                                }
                            }}
                        />
                        <Caption content={mode_text} />
                    </Section>
                    <Section @sizing=ItemSize::Percent(50.0) title="Listbox">
                        <Caption content="Type to search" />
                        <Listbox
                            options={view! {
                                <ChoiceOption label="Amber" />
                                <ChoiceOption label="Blue" />
                                <ChoiceOption label="Green" />
                                <ChoiceOption label="Purple" />
                            }}
                            selected=Some(1)
                            on_change={move |selected: Option<usize>| {
                                if let Some(index) = selected {
                                    set_color_text.set(format!("{} selected", colors[index]));
                                }
                            }}
                        />
                        <Caption content={color_text} />
                    </Section>
                </Stack>
                <Section title="Select">
                    <Select
                        options={view! {
                            <ForEach keys={FRUITS.to_vec()}>
                                {|label: &'static str| view! {
                                    <ChoiceOption label />
                                }}
                            </ForEach>
                        }}
                        selected=Some(0)
                        label="Favorite fruit"
                        on_change={move |selected: Option<usize>| {
                            let text = selected
                                .and_then(|index| FRUITS.get(index))
                                .map_or_else(
                                    || "Nothing selected".to_owned(),
                                    |label| format!("{label} selected"),
                                );
                            set_fruit_text.set(text);
                        }}
                    />
                    <Caption content={fruit_text} />
                </Section>
                <Section title="Tabs">
                    <Tabs
                        options={view! {
                            <ChoiceOption label="1x" />
                            <ChoiceOption label="2x" />
                            <ChoiceOption label="3x" />
                        }}
                        selected={scale}
                        on_change={move |selected| set_scale.set(selected)}
                    />
                    <Caption content={scale_text} />
                </Section>
                <Section title="Responsive tabs">
                    <Caption
                        content="Tabs that turn into a select when there is no room for them"
                        wrap=true
                    />
                    <ResponsiveTabs
                        options={view! {
                            <ChoiceOption label="Day" />
                            <ChoiceOption label="Week" />
                            <ChoiceOption label="Month" />
                            <ChoiceOption label="Year" />
                        }}
                        selected=0
                        breakpoint=TABS_NARROW_WIDTH
                        on_change={move |selected| set_view_tab.set(selected)}
                    />
                    <Caption content={view_text} />
                </Section>
            </List>
        </ScrollPage>
    }
}

#[component]
fn PickersPage() -> NodeId {
    let today = use_context::<Today>().map_or_else(Date::today, |Today(today)| today);
    let (day, set_day) = create_signal(Some(DateTime::new(today, Time::MIDNIGHT)));
    let (alarm, set_alarm) = create_signal(Some(DateTime::new(today, Time::new(7, 30))));
    let (meeting, set_meeting) = create_signal(None::<DateTime>);
    let (booked, set_booked) = create_signal(None::<Date>);
    let (paint, set_paint) = create_signal(Color32::from_rgb(0x3E, 0x63, 0xDD));
    let (preview, set_preview) = create_signal(None::<Color32>);
    let (accent, set_accent) =
        create_signal(Color32::from_rgba_unmultiplied(0xF7, 0x6B, 0x15, 0xC0));
    let day_text = create_memo(clone!(day -> move || match day.get() {
        Some(day) => format!("{} was chosen", day.date.label()),
        None => "No day chosen".to_owned(),
    }));
    let alarm_text = create_memo(clone!(alarm -> move || match alarm.get() {
        Some(alarm) => format!("The alarm rings at {}", alarm.time.format(HourCycle::H12)),
        None => "No alarm".to_owned(),
    }));
    let meeting_text = create_memo(clone!(meeting -> move || match meeting.get() {
        Some(meeting) => format!(
            "Meeting on {} at {}",
            meeting.date.label(),
            meeting.time.format(HourCycle::H24)
        ),
        None => "No meeting yet: type one in or pick it".to_owned(),
    }));
    let booked_text = create_memo(clone!(booked -> move || match booked.get() {
        Some(booked) => format!("Booked for {}", booked.label()),
        None => "Bookings open for the next 60 days".to_owned(),
    }));
    let shown_paint =
        create_memo(clone!(paint preview -> move || preview.get().unwrap_or(paint.get())));
    let paint_text = create_memo(clone!(shown_paint -> move || {
        format!("Painting in {}", beui::format_hex(shown_paint.get(), false))
    }));
    view! {
        <ScrollPage>
            <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                <List @sizing=ItemSize::Percent(50.0) spacing=PAGE_SPACING>
                    <Section title="Date">
                        <DateTimeField
                            value={day}
                            parts=DateTimeParts::Date
                            label="Day"
                            today={Some(today)}
                            on_change={move |next| set_day.set(next)}
                        />
                        <Caption content={day_text} />
                    </Section>
                    <Section title="Time, on a 12-hour clock">
                        <DateTimeField
                            value={alarm}
                            parts=DateTimeParts::Time
                            hour_cycle=HourCycle::H12
                            label="Alarm"
                            today={Some(today)}
                            on_change={move |next| set_alarm.set(next)}
                        />
                        <Caption content={alarm_text} />
                    </Section>
                    <Section title="Date and time, which can be cleared">
                        <DateTimeField
                            value={meeting}
                            label="Meeting"
                            today={Some(today)}
                            clearable=true
                            step_minutes=30
                            on_change={move |next| set_meeting.set(next)}
                        />
                        <Caption content={meeting_text} wrap=true />
                    </Section>
                    <Section title="Calendar">
                        <Caption content="Limited to the next 60 days" />
                        <Calendar
                            selected={booked}
                            min={Some(today)}
                            max={Some(today.add_days(60))}
                            today={Some(today)}
                            on_change={move |date| set_booked.set(Some(date))}
                        />
                        <Caption content={booked_text} />
                    </Section>
                </List>
                <List @sizing=ItemSize::Percent(50.0) spacing=PAGE_SPACING>
                    <Section title="Color input">
                        <Caption content="Click the swatch to pick" />
                        <ColorInput
                            value={accent}
                            label="Accent"
                            on_change={move |color| set_accent.set(color)}
                        />
                    </Section>
                    <Section title="Color picker">
                        <ColorPicker
                            value={paint}
                            alpha=false
                            on_change={move |color| set_paint.set(color)}
                            on_preview={move |color| set_preview.set(color)}
                        />
                        <Frame height=SWATCH_HEIGHT color={shown_paint} radius=RADIUS />
                        <Caption content={paint_text} />
                    </Section>
                </List>
            </Stack>
        </ScrollPage>
    }
}

#[component]
fn MenusPage() -> NodeId {
    let (menu_text, set_menu_text) = create_signal("Nothing chosen yet".to_owned());
    let (sort_text, set_sort_text) = create_signal("Sorted by name".to_owned());
    let (run_text, set_run_text) = create_signal("Not running".to_owned());
    let (save_text, set_save_text) = create_signal("Not saved".to_owned());
    let (copied, set_copied) = create_signal(false);
    let nothing_copied = create_memo(move || !copied.get());
    let more = set_sort_text.clone();
    let items = view! {
        <unstyled::MenuItem label="Copy" />
        <unstyled::MenuItem label="Paste" disabled={nothing_copied} />
        <unstyled::MenuItem label="Share">
            <unstyled::MenuItem label="Email" />
            <unstyled::MenuItem label="Link" />
        </unstyled::MenuItem>
    };
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Context menu">
                    <ContextMenu
                        items
                        on_select={move |path: Vec<usize>| {
                            let label = match path.as_slice() {
                                [0] => {
                                    set_copied.set(true);
                                    "Copy".to_owned()
                                }
                                [1] => "Paste".to_owned(),
                                [2, 0] => "Share > Email".to_owned(),
                                [2, 1] => "Share > Link".to_owned(),
                                other => format!("{other:?}"),
                            };
                            set_menu_text.set(format!("Chose: {label}"));
                        }}
                    >
                        <Card>
                            <List spacing=4.0>
                                <Caption content="Right-click this card" />
                                <Paragraph
                                    content="Paste stays disabled until Copy is chosen; Share opens \
                                     a submenu on hover or Right Arrow, and Left Arrow closes it."
                                />
                            </List>
                        </Card>
                    </ContextMenu>
                    <Caption content={menu_text} />
                </Section>
                <Section title="Menu buttons">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0 wrap=true>
                        <MenuButton
                            label="Sort"
                            items={view! {
                                <unstyled::MenuItem label="By name" />
                                <unstyled::MenuItem label="By date" />
                                <unstyled::MenuItem label="By size" />
                            }}
                            on_select={move |path: Vec<usize>| {
                                let by = match path.as_slice() {
                                    [0] => "name",
                                    [1] => "date",
                                    _ => "size",
                                };
                                set_sort_text.set(format!("Sorted by {by}"));
                            }}
                        />
                        <MenuButton
                            label="More"
                            glyph=ICON_MENU
                            icon_only=true
                            arrow=false
                            variant=ButtonVariant::Ghost
                            items={view! {
                                <unstyled::MenuItem label="Reverse the order" />
                            }}
                            on_select={move |_: Vec<usize>| more.set("Sorted in reverse".to_owned())}
                        />
                    </List>
                    <Caption content={sort_text} />
                </Section>
                <Section title="Split buttons">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0 wrap=true>
                        <SplitButton
                            label="Run the app"
                            glyph=ICON_PLAY_ARROW
                            variant=ButtonVariant::Primary
                            menu_label="More ways to run it"
                            items={view! {
                                <unstyled::MenuItem label="Run with fresh data" />
                                <unstyled::MenuItem label="Run on the web" />
                            }}
                            on_click={clone!(set_run_text -> move || set_run_text.set("Running the app".to_owned()))}
                            on_select={move |path: Vec<usize>| {
                                let how = match path.as_slice() {
                                    [0] => "with fresh data",
                                    _ => "on the web",
                                };
                                set_run_text.set(format!("Running the app {how}"));
                            }}
                        />
                        <SplitButton
                            label="Save"
                            variant=ButtonVariant::Secondary
                            menu_label="More ways to save"
                            items={view! {
                                <unstyled::MenuItem label="Save as a copy" />
                            }}
                            on_click={clone!(set_save_text -> move || set_save_text.set("Saved".to_owned()))}
                            on_select={move |_: Vec<usize>| set_save_text.set("Saved a copy".to_owned())}
                        />
                    </List>
                    <Caption content={run_text} />
                    <Caption content={save_text} />
                </Section>
            </List>
        </ScrollPage>
    }
}

#[component]
fn OverlaysPage() -> NodeId {
    let (dialog, set_dialog) = create_signal(false);
    let (sheet, set_sheet) = create_signal(false);
    let (fullscreen, set_fullscreen) = create_signal(false);
    let (outcome, set_outcome) = create_signal("Nothing chosen yet".to_owned());
    let open_dialog = set_dialog.clone();
    let dismiss_dialog = set_dialog.clone();
    let keep = set_dialog.clone();
    let discard = set_outcome.clone();
    let open_sheet = set_sheet.clone();
    let dismiss_sheet = set_sheet.clone();
    let duplicate = set_sheet.clone();
    let duplicated = set_outcome.clone();
    let renamed = set_outcome.clone();
    let rename = set_sheet.clone();
    let removed = set_outcome.clone();
    let open_fullscreen = set_fullscreen.clone();
    let dismiss_fullscreen = set_fullscreen.clone();
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Popover">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Popover label="Filters">
                            {move |handle: PopoverHandle| view! {
                                <FilterPopover handle />
                            }}
                        </Popover>
                    </List>
                </Section>
                <Section title="Dialog">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Button
                            label="Discard changes…"
                            variant=ButtonVariant::Secondary
                            on_click={move || open_dialog.set(true)}
                        />
                    </List>
                    <Dialog
                        open={dialog}
                        title="Discard changes?"
                        width=DIALOG_WIDTH
                        on_dismiss={move || dismiss_dialog.set(false)}
                    >
                        <List spacing=ROW_SPACING>
                            <Paragraph
                                content="Escape, a click outside or the back gesture dismisses a dialog."
                            />
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                                <Button
                                    label="Keep"
                                    variant=ButtonVariant::Secondary
                                    on_click={move || keep.set(false)}
                                />
                                <Button
                                    label="Discard"
                                    variant=ButtonVariant::Primary
                                    on_click={move || {
                                        discard.set("Discarded the changes".to_owned());
                                        set_dialog.set(false);
                                    }}
                                />
                            </List>
                        </List>
                    </Dialog>
                </Section>
                <Section title="Modal sheet">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Button
                            label="Show actions"
                            variant=ButtonVariant::Secondary
                            on_click={move || open_sheet.set(true)}
                        />
                    </List>
                    <Caption
                        content="A sheet slides up from the bottom; drag its handle, or tap above it to close it."
                        wrap=true
                    />
                    <ModalSheet open={sheet} fit=true on_close={move || dismiss_sheet.set(false)}>
                        <List spacing=0.0>
                            <ActionRow
                                label="Duplicate"
                                glyph=ICON_CONTENT_COPY
                                on_click={move || {
                                    duplicated.set("Duplicated".to_owned());
                                    duplicate.set(false);
                                }}
                            />
                            <ActionRow
                                label="Rename"
                                glyph=ICON_EDIT
                                detail="Ctrl+R"
                                on_click={move || {
                                    renamed.set("Renamed".to_owned());
                                    rename.set(false);
                                }}
                            />
                            <ActionRow label="Share" glyph=ICON_SHARE disabled=true />
                            <ActionRow
                                label="Delete"
                                glyph=ICON_DELETE
                                danger=true
                                on_click={move || {
                                    removed.set("Deleted".to_owned());
                                    set_sheet.set(false);
                                }}
                            />
                        </List>
                    </ModalSheet>
                </Section>
                <Section title="Fullscreen">
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Button
                            label="Go fullscreen"
                            variant=ButtonVariant::Secondary
                            on_click={move || open_fullscreen.set(true)}
                        />
                    </List>
                    <Fullscreen
                        open={fullscreen}
                        on_dismiss={move || dismiss_fullscreen.set(false)}
                    >
                        <FullscreenBody on_close={move || set_fullscreen.set(false)} />
                    </Fullscreen>
                </Section>
                <Caption content={outcome} />
            </List>
        </ScrollPage>
    }
}

#[component]
fn FilterPopover(handle: PopoverHandle) -> NodeId {
    let close = handle.close;
    view! {
        <List spacing=SECTION_SPACING>
            <Heading content="Filters" />
            <Checkbox label="Only starred" checked=false />
            <Checkbox label="Include archived" checked=true />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Done"
                    variant=ButtonVariant::Primary
                    on_click={move || close.call(())}
                />
            </List>
        </List>
    }
}

#[component]
fn FullscreenBody(on_close: ClickCallback) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=PAGE_PADDING
            padding_vertical=PAGE_PADDING
        >
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Title @sizing=ItemSize::Percent(100.0) content="Fullscreen" />
                    <Button
                        label="Close"
                        variant=ButtonVariant::Secondary
                        on_click={move || on_close.call()}
                    />
                </List>
                <Paragraph content="This covers the whole window. Escape closes it too." />
            </List>
        </Frame>
    }
}

#[derive(Clone)]
struct Rows {
    set_status: WriteSignal<String>,
    selection: Selector<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    timings: ReadSignal<bool>,
    set_timings: WriteSignal<bool>,
    compact: ReadSignal<bool>,
    set_compact: WriteSignal<bool>,
}

impl Rows {
    fn new(set_status: WriteSignal<String>) -> Self {
        let (selected, set_selected) = create_signal(None);
        let selection = create_selector(clone!(selected -> move || selected.get()));
        let (timings, set_timings) = create_signal(true);
        let (compact, set_compact) = create_signal(false);
        Self {
            set_status,
            selection,
            set_selected,
            timings,
            set_timings,
            compact,
            set_compact,
        }
    }

    fn select(&self, index: usize) {
        self.set_selected.set(Some(index));
        self.set_status.set(format!("Row {index} selected"));
    }
}

#[component]
fn RowsPage() -> NodeId {
    let (status, set_status) = create_signal("Nothing selected".to_owned());
    let rows = Rows::new(set_status);
    let compact = rows.compact.clone();
    let row_height = create_memo(move || match compact.get() {
        true => COMPACT_ROW_HEIGHT,
        false => ROW_HEIGHT,
    });
    let timing_rows = rows.clone();
    let compact_rows = rows.clone();
    let item_rows = rows.clone();
    view! {
        <FillPage>
            <List spacing=ROW_SPACING>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    spacing=ROW_SPACING
                    wrap=true
                >
                    <Checkbox
                        label="Show timings"
                        checked=true
                        on_change={move |checked| timing_rows.set_timings.set(checked)}
                    />
                    <LabelledSwitch
                        label="Compact rows"
                        on=false
                        on_change={move |on| compact_rows.set_compact.set(on)}
                    />
                </List>
                <Caption
                    content="A horizontal list scrolls with Shift+scroll, a sideways swipe, a touch \
                     drag, or the arrow keys once something in it has focus."
                    wrap=true
                />
                <Scroll @sizing=ItemSize::Fixed(STRIP_HEIGHT) direction=Direction::Horizontal>
                    <VirtualList
                        direction=Direction::Horizontal
                        keys={(0..STRIP_COUNT).collect::<Vec<usize>>()}
                        item_size=STRIP_ITEM_WIDTH
                    >
                        {move |index: usize| view! {
                            <StripCard index />
                        }}
                    </VirtualList>
                </Scroll>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Heading content={format!("Rows ({ROW_COUNT})")} />
                    <Caption
                        @sizing=ItemSize::Percent(100.0)
                        content={status}
                        align=TextAlign::End
                    />
                </List>
                <Separator />
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <VirtualList
                        keys={(0..ROW_COUNT).collect::<Vec<usize>>()}
                        item_size={row_height}
                    >
                        {move |index: usize| {
                            let rows = item_rows.clone();
                            view! {
                                <ScrollRow index rows />
                            }
                        }}
                    </VirtualList>
                </Scroll>
            </List>
        </FillPage>
    }
}

#[component]
fn StripCard(index: usize) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=4.0 padding_vertical=0.0>
            <Frame
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
                radius=RADIUS
                padding_horizontal=12.0
                padding_vertical=10.0
            >
                <Body content={format!("Card {index}")} align=TextAlign::Center />
            </Frame>
        </Frame>
    }
}

#[component]
fn ScrollRow(index: usize, rows: Rows) -> NodeId {
    let selected = rows.selection.memo(Some(index));
    let compact = rows.compact.clone();
    let select_rows = rows.clone();
    view! {
        <unstyled::Button
            on_click={move || select_rows.select(index)}
            content={move |handle| view! {
                <ScrollRowFace index handle selected timings={rows.timings} compact />
            }}
        />
    }
}

#[component]
fn ScrollRowFace(
    index: usize,
    handle: unstyled::ButtonHandle,
    selected: Memo<bool>,
    timings: ReadSignal<bool>,
    compact: ReadSignal<bool>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered, focused, ..
    } = handle;
    let vertical = create_memo(move || match compact.get() {
        true => COMPACT_ROW_PADDING_VERTICAL,
        false => ROW_PADDING_VERTICAL,
    });
    let theme = use_theme();
    let value_color = create_memo(clone!(selected theme -> move || {
        let theme = theme.get();
        match selected.get() {
            true => theme.accent,
            false => theme.text_muted,
        }
    }));
    let fill_color = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent_soft,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    }));
    view! {
        <Frame
            color={fill_color}
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_visible={focus_ring(focused)}
            padding_horizontal=ROW_PADDING_HORIZONTAL
            padding_vertical={vertical}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Body @sizing=ItemSize::Percent(100.0) content={format!("Row {index}")} />
                <Frame visible={timings}>
                    <Caption
                        content={format!("{} ms", 7 + index * 3 % 91)}
                        align=TextAlign::End
                        color={value_color}
                    />
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn TreePage() -> NodeId {
    let (collapsed, set_collapsed) = create_signal(Vec::<usize>::new());
    let (selected, set_selected) = create_signal(None::<usize>);
    let keys = create_memo(clone!(collapsed -> move || visible_tree_rows(&collapsed.get())));
    let status_text = create_memo(clone!(selected -> move || match selected.get() {
        Some(row) => format!("{} selected", TREE_NODES[row].0),
        None => "Nothing selected".to_owned(),
    }));
    let expansion = collapsed.clone();
    let reveal = set_collapsed.clone();
    view! {
        <ScrollPage>
            <Section title="Tree">
                <Caption
                    content="Arrow keys walk the tree and Enter selects; a chevron or an arrow key opens and closes a folder"
                    wrap=true
                />
                <Tree
                    @sizing=ItemSize::Fixed(TREE_HEIGHT)
                    keys
                    item={move |row: usize| tree_item(row, &expansion.get())}
                    selected
                    ancestors={tree_ancestors}
                    on_reveal={move |row: usize| {
                        reveal.update(|collapsed| {
                            collapsed.retain(|candidate| !tree_ancestors(row).contains(candidate));
                        });
                    }}
                    on_select={move |row: usize| set_selected.set(Some(row))}
                    on_expand={move |(row, expanded): (usize, bool)| {
                        set_collapsed.update(|collapsed| {
                            collapsed.retain(|candidate| *candidate != row);
                            if !expanded {
                                collapsed.push(row);
                            }
                        });
                    }}
                >
                    {move |row: TreeRowFace<usize>| view! {
                        <Body content={TREE_NODES[row.key].0} />
                    }}
                </Tree>
                <Caption content={status_text} />
            </Section>
        </ScrollPage>
    }
}

fn tree_item(row: usize, collapsed: &[usize]) -> TreeItem {
    let (label, depth) = TREE_NODES[row];
    TreeItem {
        label: label.to_owned(),
        depth,
        expandable: has_children(row),
        expanded: !collapsed.contains(&row),
    }
}

fn tree_ancestors(row: usize) -> Vec<usize> {
    let mut depth = TREE_NODES[row].1;
    let mut ancestors: Vec<usize> = (0..row)
        .rev()
        .filter(|candidate| {
            let above = TREE_NODES[*candidate].1 < depth;
            if above {
                depth = TREE_NODES[*candidate].1;
            }
            above
        })
        .collect();
    ancestors.reverse();
    ancestors
}

fn has_children(row: usize) -> bool {
    TREE_NODES
        .get(row + 1)
        .is_some_and(|(_, depth)| *depth > TREE_NODES[row].1)
}

fn visible_tree_rows(collapsed: &[usize]) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut hidden_below: Option<usize> = None;
    for (row, (_, depth)) in TREE_NODES.iter().enumerate() {
        match hidden_below {
            Some(limit) if *depth > limit => continue,
            _ => hidden_below = None,
        }
        rows.push(row);
        if collapsed.contains(&row) {
            hidden_below = Some(*depth);
        }
    }
    rows
}

#[component]
fn LayoutPage() -> NodeId {
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Cards in a stack">
                    <Caption
                        content="A stack lays its children side by side, and one above the other once it is narrow."
                        wrap=true
                    />
                    <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                        <Card @sizing=ItemSize::Percent(50.0)>
                            <List spacing=4.0>
                                <Caption content="Left" />
                                <Display content="42" />
                            </List>
                        </Card>
                        <Card @sizing=ItemSize::Percent(50.0)>
                            <List spacing=4.0>
                                <Caption content="Right" />
                                <Display content="17" />
                            </List>
                        </Card>
                    </Stack>
                </Section>
                <Section title="Accordions">
                    <Card>
                        <List spacing=ROW_SPACING>
                            <Accordion title="About" open=true>
                                <Paragraph
                                    content="beui keeps a retained tree of nodes. Base nodes carry \
                                     behaviour only, unstyled components compose them, and the \
                                     styled components paint them."
                                />
                            </Accordion>
                            <Separator />
                            <Accordion title="Layout" open=false>
                                <Paragraph
                                    content="Layout is linear in the number of nodes, and only the \
                                     parts of the tree whose inputs changed are laid out again."
                                />
                            </Accordion>
                        </List>
                    </Card>
                </Section>
                <Section title="Separators and borders">
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Body content="One" />
                        <Separator direction=Direction::Vertical length=TOOLBAR_RULE_LENGTH />
                        <Body content="Two" />
                        <Separator direction=Direction::Vertical length=TOOLBAR_RULE_LENGTH />
                        <Body content="Three" />
                    </List>
                    <Separator />
                    <Bordered corner_radius=CARD_RADIUS>
                        <Frame padding_horizontal=ROW_SPACING padding_vertical=ROW_SPACING>
                            <Body content="A border drawn around whatever it holds" />
                        </Frame>
                    </Bordered>
                </Section>
                <Section title="A scroll inside a scroll">
                    <Card>
                        <List spacing=0.0>
                            <Scroll @sizing=ItemSize::Fixed(NESTED_SCROLL_HEIGHT)>
                                <List spacing=4.0>
                                    <ForEach
                                        keys={(1..=NESTED_SCROLL_ROWS).collect::<Vec<usize>>()}
                                    >
                                        {move |line: usize| view! {
                                            <Body content={format!("Line {line}")} />
                                        }}
                                    </ForEach>
                                </List>
                            </Scroll>
                        </List>
                    </Card>
                </Section>
            </List>
        </ScrollPage>
    }
}

#[component]
fn EditorPage() -> NodeId {
    let document = Arc::new(TextBuffer::new(EDITOR_TEXT)) as Arc<dyn text_editor_core::Document>;
    let state = TextAreaState::new(document);
    state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
    let bold = state.clone();
    let italic = state.clone();
    view! {
        <FillPage>
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Heading @sizing=ItemSize::Percent(100.0) content="Editor" />
                    <Button
                        label="Bold"
                        variant=ButtonVariant::Secondary
                        on_click={move || bold.execute(EditorCommand::Markdown(MarkdownCommand::Bold))}
                    />
                    <Button
                        label="Italic"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            italic.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
                        }}
                    />
                </List>
                <TextArea @sizing=ItemSize::Percent(100.0) state={state} />
            </List>
        </FillPage>
    }
}

#[component]
fn CanvasPage() -> NodeId {
    let (stage_view, set_stage_view) = create_signal(STAGE_VIEW);
    let zoom_out = set_stage_view.clone();
    let zoom_in = set_stage_view.clone();
    let stage = set_stage_view.clone();
    let zoom_label = create_memo(clone!(stage_view -> move || {
        format!("{:.0}%", stage_view.get().scale * 100.0)
    }));
    view! {
        <FillPage>
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Heading @sizing=ItemSize::Percent(100.0) content="Canvas" />
                    <Caption content={zoom_label} />
                    <Separator direction=Direction::Vertical length=TOOLBAR_RULE_LENGTH />
                    <Button
                        @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                        label="-"
                        variant=ButtonVariant::Secondary
                        on_click={move || zoom_out.update(scale_out)}
                    />
                    <Button
                        @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                        label="+"
                        variant=ButtonVariant::Secondary
                        on_click={move || zoom_in.update(scale_in)}
                    />
                    <Button
                        label="Reset"
                        variant=ButtonVariant::Secondary
                        on_click={move || set_stage_view.set(STAGE_VIEW)}
                    />
                </List>
                <Separator />
                <CanvasStage
                    @sizing=ItemSize::Percent(100.0)
                    view={stage_view}
                    on_change={move |view| stage.set(view)}
                />
                <Caption
                    wrap=true
                    content="Scroll to pan, Shift+scroll sideways, Ctrl+scroll or pinch to zoom, \
                     and drag with the middle button. Tab to it for arrows, + and -."
                />
            </List>
        </FillPage>
    }
}

fn scale_out(view: &mut PanZoomView) {
    view.scale = (view.scale / STAGE_ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE);
}

fn scale_in(view: &mut PanZoomView) {
    view.scale = (view.scale * STAGE_ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE);
}

#[component]
fn CanvasStage(view: ReadSignal<PanZoomView>, on_change: Callback<PanZoomView>) -> NodeId {
    view! {
        <PanZoom view on_change={move |view| on_change.call(view)}>
            {move |handle: PanZoomHandle| {
                let PanZoomHandle { view, scale, focused, .. } = handle;
                view! {
                    <CanvasBoard view scale focused />
                }
            }}
        </PanZoom>
    }
}

#[component]
fn CanvasBoard(
    view: Memo<Option<CanvasView>>,
    scale: Memo<f32>,
    focused: ReadSignal<bool>,
) -> NodeId {
    let [first, second, third, fourth] = STAGE_CARDS;
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focus_ring(focused)}
            radius=RADIUS
        >
            <Canvas view>
                <StageCard card=first scale={scale.clone()} />
                <StageCard card=second scale={scale.clone()} />
                <StageCard card=third scale={scale.clone()} />
                <StageCard card=fourth scale />
            </Canvas>
        </Frame>
    }
}

#[component]
fn StageCard(card: (f32, f32, f32, f32, &'static str), scale: Memo<f32>) -> CanvasItem {
    let (x, y, width, height, label) = card;
    let theme = use_theme();
    let font_size = create_memo(clone!(scale -> move || STAGE_LABEL_SIZE * scale.get()));
    let padding = create_memo(clone!(scale -> move || STAGE_PADDING * scale.get()));
    view! {
        <CanvasItem x y width height>
            <Frame
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
                radius=CARD_RADIUS
                padding_horizontal={padding.clone()}
                padding_vertical={padding}
            >
                <Text string={label.to_string()} font_size={font_size} color={theme.text.clone()} />
            </Frame>
        </CanvasItem>
    }
}

#[component]
fn ThemesPage() -> NodeId {
    view! {
        <ScrollPage>
            <List spacing=PAGE_SPACING>
                <Section title="Themes">
                    <Paragraph
                        content="E-ink, in the toolbar, changes the theme of the whole document. \
                         A theme provider restyles only what it holds, as the two samples below show."
                    />
                </Section>
                <Stack spacing=COLUMN_SPACING breakpoint=COLUMNS_BREAKPOINT>
                    <ThemeProvider @sizing=ItemSize::Percent(50.0) theme=Theme::DARK>
                        <ThemeSample name="Dark" />
                    </ThemeProvider>
                    <ThemeProvider @sizing=ItemSize::Percent(50.0) theme=Theme::EINK>
                        <ThemeSample name="E-ink" />
                    </ThemeProvider>
                </Stack>
            </List>
        </ScrollPage>
    }
}

#[component]
fn ThemeSample(name: &'static str) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            radius=CARD_RADIUS
            padding_horizontal=ROW_SPACING
            padding_vertical=ROW_SPACING
        >
            <Card>
                <List spacing=SECTION_SPACING>
                    <Heading content={name} />
                    <Caption
                        content="Every styled component reads its colors from the theme."
                        wrap=true
                    />
                    <Checkbox label="A checkbox" checked=true />
                    <LabelledSwitch label="A switch" on=true />
                    <Slider value=0.6 label="A slider" />
                    <Progress value=0.6 />
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0 wrap=true>
                        <Button label="Primary" variant=ButtonVariant::Primary />
                        <Button label="Secondary" variant=ButtonVariant::Secondary />
                    </List>
                </List>
            </Card>
        </Frame>
    }
}

#[cfg(test)]
mod tests;
