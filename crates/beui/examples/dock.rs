use beui::reactive::{
    ForEach, Frame, Func, List, ReadSignal, Show, Spacer, WriteSignal, build, clone, component,
    create_memo, create_signal, view,
};
use beui::styled::theme::use_theme;
use beui::styled::{
    Body, Button, ButtonVariant, Caption, DockArea, Heading, ListRow, Paragraph, Scroll, Separator,
    Title,
};
use beui::unstyled::{DockState, Side, TabId};
use beui::{Align, Color32, Context, Direction, Document, ItemSize, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui dock", DockDemo::new())
}

const FILES: TabId = TabId::new(1);
const EMPTY: TabId = TabId::new(2);
const FILES_SHARE: f32 = 0.78;
const SHELL_PADDING: f32 = 10.0;
const SHELL_SPACING: f32 = 10.0;
const PANEL_PADDING: f32 = 14.0;
const ROW_SPACING: f32 = 4.0;
const TOOLBAR_SPACING: f32 = 8.0;
const SEPARATOR_HEIGHT: f32 = 1.0;
const NEXT_TAB: u64 = 100;

#[derive(Clone, PartialEq)]
struct Paper {
    tab: TabId,
    title: String,
    body: String,
}

const PAPERS: [(u64, &str, &str); 5] = [
    (
        3,
        "Welcome",
        "Drag a tab by its label. Drop it over the middle of a pane to join that pane, \
         over an edge to split it, or over a tab bar to land between the tabs there.",
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
        "The tab bar is a tab list: the arrow keys walk it, Home and End jump to its ends, \
         and the counter below keeps its value while you switch between tabs.",
    ),
    (
        7,
        "Notes",
        "Closing a tab leaves the paper in Files, so it can be opened again.",
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
    state
}

fn settled(mut state: DockState) -> DockState {
    let open = state
        .all_tabs()
        .into_iter()
        .any(|tab| tab != FILES && tab != EMPTY);
    if open {
        state.remove(EMPTY);
        return state;
    }
    if state.contains(EMPTY) {
        return state;
    }
    let files = state.find(FILES).map(|position| position.leaf);
    let elsewhere = state
        .surfaces()
        .into_iter()
        .flat_map(|surface| state.leaves(surface))
        .find(|leaf| Some(*leaf) != files);
    match (elsewhere, files) {
        (Some(leaf), _) => state.push(leaf, EMPTY),
        (None, Some(files)) => {
            state.split(files, Side::Right, FILES_SHARE, vec![EMPTY]);
        }
        (None, None) => state.push_to_focused(EMPTY),
    }
    state
}

fn open(state: &mut DockState, tab: TabId) {
    if state.contains(tab) {
        state.show(tab);
        return;
    }
    if state.replace(EMPTY, tab) {
        state.show(tab);
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
}

#[component]
fn DockShell() -> NodeId {
    let theme = use_theme();
    let (papers, set_papers) = create_signal(papers());
    let (state, set_state) = create_signal(starting_state());
    let title = Func::new(clone!(papers -> move |tab: TabId| match tab {
        FILES => "Files".to_owned(),
        EMPTY => "Workspace".to_owned(),
        tab => papers.with(|papers| {
            papers
                .iter()
                .find(|paper| paper.tab == tab)
                .map_or_else(|| "Untitled".to_owned(), |paper| paper.title.clone())
        }),
    }));
    let content_papers = papers.clone();
    let content_state = set_state.clone();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=SHELL_PADDING
            padding_vertical=SHELL_PADDING
        >
            <List spacing=SHELL_SPACING>
                <DockToolbar set_papers={set_papers} set_state={set_state.clone()} />
                <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                <DockArea
                    @sizing=ItemSize::Percent(100.0)
                    state={state}
                    title={title}
                    closable={Func::new(|tab: TabId| tab != FILES && tab != EMPTY)}
                    on_change={move |next: DockState| set_state.set(settled(next))}
                    on_close={move |_: TabId| {}}
                >
                    {move |tab: TabId| {
                        let papers = content_papers.clone();
                        let set_state = content_state.clone();
                        match tab {
                            FILES => view! {
                                <FilesPanel papers set_state />
                            },
                            EMPTY => view! {
                                <EmptyPanel />
                            },
                            tab => view! {
                                <PaperPanel tab papers />
                            },
                        }
                    }}
                </DockArea>
            </List>
        </Frame>
    }
}

#[component]
fn DockToolbar(set_papers: WriteSignal<Vec<Paper>>, set_state: WriteSignal<DockState>) -> NodeId {
    let (next, set_next) = create_signal(NEXT_TAB);
    let added = set_state.clone();
    let reset = set_state.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TOOLBAR_SPACING>
            <Title content="Workspace" />
            <Spacer @sizing=ItemSize::Percent(100.0) />
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

fn title_of(papers: &ReadSignal<Vec<Paper>>, tab: TabId) -> String {
    papers.with(|papers| {
        papers
            .iter()
            .find(|paper| paper.tab == tab)
            .map_or_else(|| "Untitled".to_owned(), |paper| paper.title.clone())
    })
}
