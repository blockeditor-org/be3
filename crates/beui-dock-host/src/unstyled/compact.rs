use super::*;

const STAGE: u64 = 0;
const SPLITTER: u64 = 1;
const GATHERED: u64 = 2;

#[component]
pub(super) fn DockCompactView(dock: State, tree: Tree) -> NodeId {
    let state = dock.state;
    let area = create_signal(Rect::ZERO);
    let portrait = create_memo(move || {
        let size = area.get().size();
        size.y > size.x
    });
    let stage = create_memo(move || state.with(|state| state.stage_of(tree)));
    let gathered = create_memo(move || state.with(|state| !state.gathered_tabs(tree).is_empty()));
    let folded = create_memo(move || gathered.get() && state.with(|state| state.is_folded(tree)));
    let arranged = create_memo(move || (portrait.get(), folded.get(), stage.get(), gathered.get()));
    let built = view! {
        <List spacing=0.0>
            <Dynamic
                value={arranged}
            >|(portrait, folded, stage, gathered): (bool, bool, Option<LeafId>, bool)| {
                let dock = dock;
                let stage = match stage {
                    Some(leaf) => view! {
                        <DockPanelView dock tree leaf hoisted=false />
                    },
                    None => view! {
                        <Frame />
                    },
                };
                let direction = match portrait {
                    true => Direction::Vertical,
                    false => Direction::Horizontal,
                };
                match (gathered, folded) {
                    (false, _) => view! {
                        <ListItem grow=1.0>
                            <List spacing=0.0>
                                <ListItem grow=1.0>{stage}</ListItem>
                            </List>
                        </ListItem>
                    },
                    (true, true) => {
                        let across = (!portrait).then_some(dock.icon_sidebar_width);
                        view! {
                            <ListItem grow=1.0>
                                <List direction spacing=0.0>
                                    <ListItem grow=1.0>{stage}</ListItem>
                                    <ListItem length=across>
                                        <DockGathered dock tree vertical={!portrait} folded=true />
                                    </ListItem>
                                </List>
                            </ListItem>
                        }
                    }
                    (true, false) => {
                        let wanted = create_memo(move || {
                            state.with(|state| state.compact_wanted(tree, portrait))
                        });
                        let thickness = dock.thickness;
                        let reach = dock.reach;
                        let arrangement = create_memo(move || {
                            let wanted = wanted.get();
                            Some(CanvasArrangement::new(move |size| {
                                compact_rects(portrait, wanted, size, thickness, reach)
                            }))
                        });
                        view! {
                            <ListItem grow=1.0>
                                <Canvas arrangement>
                                    <CanvasItem arranged={STAGE}>{stage}</CanvasItem>
                                    <CanvasItem arranged={GATHERED}>
                                        <DockGathered dock tree vertical=false folded=false />
                                    </CanvasItem>
                                    <CanvasItem arranged={SPLITTER}>
                                        <DockCompactSplitter
                                            dock
                                            tree
                                            direction
                                            wanted
                                            area={area.read_only()}
                                        />
                                    </CanvasItem>
                                </Canvas>
                            </ListItem>
                        }
                    }
                }
            }</Dynamic>
        </List>
    };
    let measured = measure(built);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            area.set(measured.rect);
        }
    });
    built
}

fn compact_rects(
    portrait: bool,
    wanted: Option<f32>,
    size: Vec2,
    thickness: f32,
    reach: f32,
) -> Vec<(u64, Rect)> {
    let (length, across) = match portrait {
        true => (size.y, size.x),
        false => (size.x, size.y),
    };
    let gathered = compact_length(wanted, (length - thickness).max(0.0));
    let stage = (length - thickness - gathered).max(0.0);
    let rect = |start: f32, length: f32| match portrait {
        true => Rect::from_min_size(pos2(0.0, start), vec2(across, length)),
        false => Rect::from_min_size(pos2(start, 0.0), vec2(length, across)),
    };
    vec![
        (STAGE, rect(0.0, stage)),
        (GATHERED, rect(stage + thickness, gathered)),
        (SPLITTER, rect(stage - reach, thickness + 2.0 * reach)),
    ]
}

#[component]
fn DockCompactSplitter(
    dock: State,
    tree: Tree,
    direction: Direction,
    wanted: Memo<Option<f32>>,
    area: ReadSignal<Rect>,
) -> NodeId {
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let portrait = direction == Direction::Vertical;
    let thickness = dock.thickness;
    let available = move || (direction.main(area.get_untracked().size()) - thickness).max(0.0);
    let length = move || compact_length(wanted.get_untracked(), available());
    let resize = move |next: f32| {
        let next = compact_length(Some(next), available());
        dock.drag_edit(move |state| state.set_compact_size(tree, portrait, next));
    };
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Splitter);
        let available = (direction.main(area.get().size()) - thickness).max(0.0);
        node.set_numeric_value(f64::from(compact_length(wanted.get(), available)));
        node.set_min_numeric_value(0.0);
        node
    }));
    let face = dock.splitter.call(DockSplitterHandle {
        direction,
        hovered: hovered.read_only(),
        active: active.read_only(),
        focused: focused.read_only(),
    });
    let held: Rc<Cell<Option<(f32, Pos2)>>> = Rc::new(Cell::new(None));
    let grabbed = held.clone();
    let node = view! {
        <Interactive
            @test_id="dock.compact.splitter"
            capture_presses=true
            focusable=true
            on_focus_change={focused}
            on_key={move |press: KeyPress| {
                if !press.pressed {
                    return false;
                }
                let step = match (direction, press.key) {
                    (Direction::Horizontal, Key::ArrowLeft) => SIDEBAR_STEP,
                    (Direction::Horizontal, Key::ArrowRight) => -SIDEBAR_STEP,
                    (Direction::Vertical, Key::ArrowUp) => SIDEBAR_STEP,
                    (Direction::Vertical, Key::ArrowDown) => -SIDEBAR_STEP,
                    _ => return false,
                };
                resize(length() + step);
                true
            }}
            cursor={match direction {
                Direction::Horizontal => CursorIcon::ResizeHorizontal,
                Direction::Vertical => CursorIcon::ResizeVertical,
            }}
            touch_drags={TouchDrags::Along(direction)}
            on_hover_change={hovered}
            on_active_change={active}
            on_press={move |press: PointerPress| {
                grabbed.set(Some((length(), press.pos)));
            }}
            on_drag={move |press: PointerPress| {
                let Some((start, from)) = held.get() else {
                    return;
                };
                let moved = direction.main(press.pos - from);
                if moved == 0.0 {
                    return;
                }
                resize(start - moved);
            }}
        >
            <Frame>{face}</Frame>
        </Interactive>
    };
    above_embeds(node, Some(AboveEmbeds::Reaches));
    node
}

#[component]
fn DockGathered(dock: State, tree: Tree, vertical: bool, folded: bool) -> NodeId {
    let state = dock.state;
    let tabs = create_memo(move || state.with(|state| state.gathered_tabs(tree)));
    let shown = create_memo(move || state.with(|state| state.gathered_shown(tree)));
    let direction = match vertical {
        true => Direction::Vertical,
        false => Direction::Horizontal,
    };
    let list = view! {
        <List direction spacing={dock.tab_spacing}>
            <ForEach keys={tabs}>|tab: TabId| {
                let dock = dock;
                view! {
                    <DockGatheredTab dock tab shown vertical />
                }
            }</ForEach>
        </List>
    };
    let strip = dock.strip.call(DockStripHandle {
        direction,
        tabs: list,
    });
    let focused = create_memo(move || {
        let shown = shown.get();
        shown.is_some() && state.with(|state| state.focused_tab()) == shown
    });
    let bar = DockBarHandle {
        floating: false,
        vertical,
        icons_only: create_memo(move || vertical),
        focused,
        grip: view! {
            <Frame />
        },
        tabs: Some(strip),
        title: create_memo(String::new),
        vacant: create_memo(move || tabs.with(Vec::is_empty)),
        closable: create_memo(|| false),
        menu: create_memo(Vec::new),
        close: ClickCallback::new(|| {}),
        test_prefix: "dock.compact".to_owned(),
    };
    let body = match folded {
        true => view! {
            <Frame />
        },
        false => view! {
            <DockShownBody dock leaf={None} shown />
        },
    };
    let narrow = dock.icon_sidebar_width;
    let parts = ChromeParts {
        floating: false,
        vertical,
        focused,
        sidebar_width: create_memo(move || narrow),
        sidebar_splitter: None,
        bar: Some(bar),
        bar_shown: create_memo(|| true),
        leaf: None,
        body,
    };
    view! {
        <Frame @test_id="dock.compact">
            <DockChrome dock parts />
        </Frame>
    }
}

#[component]
fn DockGatheredTab(dock: State, tab: TabId, shown: Memo<Option<TabId>>, vertical: bool) -> NodeId {
    let state = dock.state;
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let selected = create_memo(move || shown.get() == Some(tab));
    let position = state.with_untracked(|state| state.find(tab));
    let leaf = position.map_or_else(|| state.with_untracked(vacant_leaf), |found| found.leaf);
    let face = dock.tab.call(DockTabHandle {
        entry: Entry::Tab(tab),
        leaf,
        index: position.map_or(0, |found| found.index),
        floating: false,
        pinned: false,
        vertical,
        icons_only: create_memo(move || vertical),
        title: create_memo(move || dock.title(tab)),
        icon: create_memo(move || dock.icon(tab)),
        picture: create_memo(move || dock.picture.call(tab)),
        tabs: create_memo(move || vec![tab]),
        has_next: create_memo(|| false),
        selected,
        hovered: hovered.read_only(),
        active: active.read_only(),
        focused: focused.read_only(),
        dragged: create_memo(|| false),
        closable: create_memo(move || dock.closable.call(tab)),
        waiting: create_memo(move || dock.waiting(&[tab])),
        host: None,
        test_key: Some(dock.test_key(tab)),
        grouped: false,
        notice: create_memo(move || dock.notice(tab)),
        close: ClickCallback::new(move || dock.close_tab(tab)),
    });
    let handle = NodeRef::new();
    let handles = dock.handles;
    handles.update(|handles| {
        handles.insert(tab, (handle, false));
    });
    on_cleanup(move || {
        if handles.is_alive() {
            handles.update(|handles| handles.retain(|_, (held, _)| *held != handle));
        }
    });
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Tab);
        node.set_label(dock.title(tab));
        node.set_selected(selected.get());
        node
    }));
    let pick = ClickCallback::new(move || {
        dock.edit(|state| state.pick_gathered(tab));
        dock.release_dialog_focus();
    });
    view! {
        <Interactive
            @node_ref=&handle
            @test_id={format!("dock.tab.{}", dock.test_key(tab))}
            focusable=true
            on_focus_change={focused}
            on_hover_change={hovered}
            on_active_change={active}
            on_click={pick}
            on_activate={pick}
            children={face}
        />
    }
}
