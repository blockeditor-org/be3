use beui::reactive::{
    BackHandler, ForEach, Frame, Func, List, Memo, ReadSignal, Show, Spacer, WriteSignal, build,
    clone, component, create_memo, create_signal, view,
};
use beui::styled::DocumentTheme;
use beui::styled::theme::use_theme;
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Checkbox, DockArea, Heading, ListRow, Paragraph, Scroll,
    Separator, Title,
};
use beui::unstyled::{Container, DockMode, DockState, Side, TabId, narrower_than};
use beui::{Align, Color32, Context, Direction, Document, ItemSize, NodeId, Rect, Vec2, pos2};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui dock", DockDemo::new())
}

const FILES: TabId = TabId::new(1);
const FILES_SHARE: f32 = 0.78;
const SHELL_PADDING: f32 = 10.0;
const SHELL_SPACING: f32 = 10.0;
const PANEL_PADDING: f32 = 14.0;
const ROW_SPACING: f32 = 4.0;
const TOOLBAR_SPACING: f32 = 8.0;
const TOOLBAR_BREAKPOINT: f32 = 340.0;
const NEXT_TAB: u64 = 100;
const SWATCH: TabId = TabId::new(9);
const SWATCH_COLOR: Color32 = Color32::from_rgb(0x2f, 0x9e, 0x6e);
const SWATCH_TEXT: Color32 = Color32::from_rgb(0x0b, 0x1f, 0x16);
const SWATCH_WINDOW: Rect = Rect::from_min_max(pos2(40.0, 360.0), pos2(340.0, 580.0));

#[derive(Clone, PartialEq)]
struct Paper {
    tab: TabId,
    title: String,
    body: String,
}

const PAPERS: [(u64, &str, &str); 7] = [
    (
        3,
        "Welcome",
        "Drag a tab by its label. Drop it over the middle of a pane to join that pane, \
         over an edge to split it, or over a tab bar to land between the tabs there. \
         With a finger, pull the tab down out of its bar first: sliding along the bar \
         scrolls it.",
    ),
    (
        4,
        "Windows",
        "Hold Alt while dragging a tab, or right-click one and pop it out, to float it in \
         a window. A window moves by the grip at the left of its bar and resizes from \
         any edge.",
    ),
    (
        5,
        "Splits",
        "Drag the bar between two panes to resize them, or focus it with Tab and use the \
         arrow keys.",
    ),
    (
        6,
        "Tabs",
        "The tab bar is a tab list: the arrow keys walk it and Home and End jump to its \
         ends. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of whichever pane you are in, \
         and the counter below keeps its value while you switch between tabs. Drag the grip \
         at the start of a pane's bar to move the whole pane, or right-click it to move \
         the tabs into a sidebar and back.",
    ),
    (
        7,
        "Notes",
        "Closing a tab leaves the paper in Files, so it can be opened again.",
    ),
    (
        8,
        "Groups",
        "Drop a tab onto the middle of another tab to group them: the group becomes one \
         tab, and choosing it shows a second tab bar holding the tabs inside. Drag a tab \
         to the edge of a group to split the group, or right-click a tab to group it or \
         split it with the tab after it. A group ungroups from its own right-click menu, \
         and one left holding a single tab turns back into that tab.",
    ),
    (
        9,
        "Swatch",
        "Opens in a window of its own and fills it edge to edge, so any gap between the \
         panel and the chrome around it shows.",
    ),
];

struct DockDemo {
    document: Document,
}

impl DockDemo {
    fn new() -> Self {
        Self {
            document: build(|| {
                view! {
                    <DockShell />
                }
            }),
        }
    }
}

impl beui::App for DockDemo {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}

fn papers() -> Vec<Paper> {
    PAPERS
        .iter()
        .map(|(id, title, body)| Paper {
            tab: TabId::new(*id),
            title: (*title).to_owned(),
            body: (*body).to_owned(),
        })
        .collect()
}

fn starting_state() -> DockState {
    let mut state = DockState::new([FILES]);
    let files = state.leaves(state.main())[0];
    state.split(
        files,
        Side::Right,
        FILES_SHARE,
        vec![TabId::new(PAPERS[0].0)],
    );
    state.open_window(SWATCH_WINDOW, vec![SWATCH]);
    state
}

fn settled(mut state: DockState) -> DockState {
    let open = state
        .all_tabs()
        .into_iter()
        .any(|tab| tab != FILES && tab != SWATCH);
    if open {
        state.remove_empty_panes();
        return state;
    }
    let files = state.find(FILES).map(|position| position.leaf);
    let elsewhere = state
        .leaves(state.main())
        .into_iter()
        .any(|leaf| Some(leaf) != files);
    if let (false, Some(files)) = (elsewhere, files) {
        state.split(files, Side::Right, FILES_SHARE, Vec::new());
    }
    state
}

fn open(state: &mut DockState, tab: TabId) {
    if state.contains(tab) {
        state.show(tab);
        return;
    }
    if tab == SWATCH {
        state.open_window(SWATCH_WINDOW, vec![SWATCH]);
        return;
    }
    let files = state.find(FILES).map(|position| position.leaf);
    let elsewhere = state
        .surfaces()
        .into_iter()
        .flat_map(|surface| state.leaves(surface))
        .find(|leaf| Some(*leaf) != files);
    let target = state
        .focused_leaf()
        .filter(|leaf| Some(*leaf) != files)
        .or(elsewhere);
    match (target, files) {
        (Some(leaf), _) => state.push(leaf, tab),
        (None, Some(files)) => {
            state.split(files, Side::Right, FILES_SHARE, vec![tab]);
        }
        (None, None) => state.push_to_focused(tab),
    }
    state.show(tab);
}

fn close(state: &mut DockState, tab: TabId) {
    state.remove(tab);
    *state = settled(state.clone());
    let next = state.recent_tabs().first().copied().unwrap_or(FILES);
    state.show(next);
}

#[component]
fn DockShell() -> NodeId {
    let theme = use_theme();
    let (papers, set_papers) = create_signal(papers());
    let (state, set_state) = create_signal(starting_state());
    let (mobile, set_mobile) = create_signal(false);
    let mode = create_memo(clone!(mobile -> move || match mobile.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    }));
    let shown = create_memo(clone!(state -> move || state.with(DockState::stacked_tab)));
    let away = create_memo(clone!(mobile shown -> move || {
        mobile.get() && shown.get().is_some_and(|tab| tab != FILES)
    }));
    let title = Func::new(clone!(papers -> move |tab: TabId| tab_title(&papers, tab)));
    let content_papers = papers.clone();
    let content_state = set_state.clone();
    let toolbar_state = set_state.clone();
    let bar_state = set_state.clone();
    let back_state = set_state.clone();
    let bar_papers = papers.clone();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=SHELL_PADDING
            padding_vertical=SHELL_PADDING
        >
            <List spacing=SHELL_SPACING>
                <Container>
                    {move |_: ReadSignal<Vec2>| {
                        let set_papers = set_papers.clone();
                        let set_state = toolbar_state.clone();
                        let mobile = mobile.clone();
                        let set_mobile = set_mobile.clone();
                        view! {
                            <DockToolbar set_papers set_state mobile set_mobile />
                        }
                    }}
                </Container>
                <Separator />
                <Show condition={away.clone()}>
                    <StackBar shown={shown} papers={bar_papers} set_state={bar_state} />
                </Show>
                <BackHandler
                    @sizing=ItemSize::Percent(100.0)
                    enabled={away}
                    on_back={move || back_state.update(|state| state.show(FILES))}
                >
                    <DockArea
                        state={state}
                        mode={mode}
                        title={title}
                        closable={Func::new(|tab: TabId| tab != FILES)}
                        on_change={move |next: DockState| set_state.set(settled(next))}
                        on_close={move |_: TabId| {}}
                        empty={move || view! {
                            <EmptyPanel />
                        }}
                    >
                        {move |tab: TabId| {
                            let papers = content_papers.clone();
                            let set_state = content_state.clone();
                            match tab {
                                FILES => view! {
                                    <FilesPanel papers set_state />
                                },
                                SWATCH => view! {
                                    <SwatchPanel />
                                },
                                tab => view! {
                                    <PaperPanel tab papers />
                                },
                            }
                        }}
                    </DockArea>
                </BackHandler>
            </List>
        </Frame>
    }
}

#[component]
fn DockToolbar(
    set_papers: WriteSignal<Vec<Paper>>,
    set_state: WriteSignal<DockState>,
    mobile: ReadSignal<bool>,
    set_mobile: WriteSignal<bool>,
) -> NodeId {
    let (next, set_next) = create_signal(NEXT_TAB);
    let added = set_state.clone();
    let reset = set_state.clone();
    let narrow = narrower_than(TOOLBAR_BREAKPOINT);
    let direction = create_memo(clone!(narrow -> move || match narrow.get() {
        true => Direction::Vertical,
        false => Direction::Horizontal,
    }));
    let align = create_memo(clone!(narrow -> move || match narrow.get() {
        true => Align::Start,
        false => Align::Center,
    }));
    let wide = create_memo(move || !narrow.get());
    view! {
        <List direction={direction} align={align} spacing=TOOLBAR_SPACING>
            <Title content="Workspace" />
            <Show condition={wide}>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
                <Checkbox
                    label="Mobile"
                    checked={mobile}
                    on_change={move |on: bool| set_mobile.set(on)}
                />
                <Button
                    label="New paper"
                    variant=ButtonVariant::Primary
                    on_click={move || {
                        let tab = TabId::new(next.get_untracked());
                        set_next.update(|next| *next += 1);
                        set_papers.update(|papers| {
                            papers.push(Paper {
                                tab,
                                title: format!("Paper {}", papers.len() + 1),
                                body: "A new paper, opened in whichever pane had the focus.".to_owned(),
                            });
                        });
                        added.update(|state| open(state, tab));
                        added.update(|state| *state = settled(state.clone()));
                    }}
                />
                <Button
                    label="Reset layout"
                    variant=ButtonVariant::Secondary
                    on_click={move || reset.set(settled(starting_state()))}
                />
            </List>
        </List>
    }
}

#[component]
fn StackBar(
    shown: Memo<Option<TabId>>,
    papers: ReadSignal<Vec<Paper>>,
    set_state: WriteSignal<DockState>,
) -> NodeId {
    let title = create_memo(clone!(shown -> move || {
        shown.get().map_or_else(String::new, |tab| tab_title(&papers, tab))
    }));
    let back = set_state.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <Button
                label="Files"
                variant=ButtonVariant::Secondary
                on_click={move || back.update(|state| state.show(FILES))}
            />
            <Heading content={title} />
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Button
                label="Close"
                variant=ButtonVariant::Secondary
                on_click={move || {
                    if let Some(tab) = shown.get_untracked() {
                        set_state.update(|state| close(state, tab));
                    }
                }}
            />
        </List>
    }
}

#[component]
fn FilesPanel(papers: ReadSignal<Vec<Paper>>, set_state: WriteSignal<DockState>) -> NodeId {
    let tabs = create_memo(clone!(papers -> move || {
        papers.with(|papers| papers.iter().map(|paper| paper.tab).collect::<Vec<_>>())
    }));
    view! {
        <Frame padding_horizontal=ROW_SPACING padding_vertical=ROW_SPACING>
            <Scroll>
                <ForEach keys={tabs}>
                    {move |tab: TabId| {
                        let papers = papers.clone();
                        let set_state = set_state.clone();
                        let title = create_memo(move || title_of(&papers, tab));
                        view! {
                            <ListRow
                                on_click={move || {
                                    set_state.update(|state| {
                                        open(state, tab);
                                        *state = settled(state.clone());
                                    });
                                }}
                            >
                                <Body content={title} />
                            </ListRow>
                        }
                    }}
                </ForEach>
            </Scroll>
        </Frame>
    }
}

#[component]
fn EmptyPanel() -> NodeId {
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=ROW_SPACING align=Align::Center>
                <Heading content="Nothing open" />
                <Caption content="Open a paper from Files to get started." />
            </List>
        </Frame>
    }
}

#[component]
fn SwatchPanel() -> NodeId {
    view! {
        <Frame color=SWATCH_COLOR padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=ROW_SPACING>
                <Heading content="Swatch" color=SWATCH_TEXT />
                <Caption
                    content="This fill should reach every edge of the pane."
                    color=SWATCH_TEXT
                />
            </List>
        </Frame>
    }
}

#[component]
fn PaperPanel(tab: TabId, papers: ReadSignal<Vec<Paper>>) -> NodeId {
    let (count, set_count) = create_signal(0u32);
    let title = create_memo(clone!(papers -> move || title_of(&papers, tab)));
    let body = create_memo(clone!(papers -> move || {
        papers.with(|papers| {
            papers
                .iter()
                .find(|paper| paper.tab == tab)
                .map_or_else(String::new, |paper| paper.body.clone())
        })
    }));
    let counted = create_memo(clone!(count -> move || format!("{} edits", count.get())));
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=SHELL_SPACING>
                <Heading content={title} />
                <Paragraph content={body} />
                <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
                    <Button
                        label="Edit"
                        variant=ButtonVariant::Secondary
                        on_click={move || set_count.update(|count| *count += 1)}
                    />
                    <Caption content={counted} />
                </List>
                <Show condition={create_memo(move || count.get() > 0)}>
                    <Caption content="Drag this tab somewhere else: the count comes with it." />
                </Show>
            </List>
        </Frame>
    }
}

fn tab_title(papers: &ReadSignal<Vec<Paper>>, tab: TabId) -> String {
    match tab {
        FILES => "Files".to_owned(),
        tab => title_of(papers, tab),
    }
}

fn title_of(papers: &ReadSignal<Vec<Paper>>, tab: TabId) -> String {
    papers.with(|papers| {
        papers
            .iter()
            .find(|paper| paper.tab == tab)
            .map_or_else(|| "Untitled".to_owned(), |paper| paper.title.clone())
    })
}
