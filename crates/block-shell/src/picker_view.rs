use beui::Overflow;
use block_editor_beui::beui::icons::{
    ICON_ARROW_LEFT, ICON_BLOCKS, ICON_CHEVRON_RIGHT, ICON_FILE_UP, ICON_FOLDER,
    ICON_LAYOUT_TEMPLATE, ICON_LINK, ICON_SEARCH,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use block_editor_beui::beui::{
    Color32, Dismiss, LayerPolicy, NodeId, OverlayAnchor, Placement, Pos2, Rect, TextAlign, Vec2,
};
use block_editor_beui::block_ui::TemplateEntry;
use block_editor_beui::{
    BlockParent, ChildMode, ChildState, Subregion, SubregionContent, label_glyph,
};
use uuid::Uuid;

use super::picker::{
    Creating, Pick, PickAction, PickTree, Stage, TreeKey, TreeRow, TypeGroup, common, groups,
    link_rows, place_rows, slides, tile_key,
};
use super::picking::Picker;

const MENU_WIDTH: Size = Size::Custom(256.0);
const SURFACE_SIZE: Vec2 = Vec2::new(520.0, 560.0);
const DIALOG_HEIGHT: Size = Size::Custom(460.0);
const SIDE_BLEED: Bleed = Bleed::DIALOG.top(false).bottom(false);
const HEADER_SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Md;
const GROUP_SPACING: Space = Space::Xs;
const CREATION_HEIGHT: f32 = 96.0;
const CREATION_INSET: Space = Space::Sm;

type Act = Callback<PickAction>;

#[component]
pub fn PickerView(picker: Picker) -> NodeId {
    let held = picker.held();
    let act = act_on(picker);
    let error = create_memo(move || held.get().and_then(|held| held.error));
    let templates = held
        .get_untracked()
        .is_some_and(|held| held.filter.templates);
    let placing = held
        .get_untracked()
        .is_some_and(|held| held.place.is_some());
    let menu = NodeRef::new();
    let failure = NodeRef::new();
    if !picker.sheet {
        fit(picker, held, menu, failure);
        close_when_left(picker, held);
    }
    let surface = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::surface_open)));
    view! {
        <BackHandler enabled={surface} on_back={move || close_surface(picker)}>
            <List spacing=Space::None>
                <PickerMenu picker held act templates placing overlay=menu />
                <PickerCard picker held />
                <CreationChild picker held />
                <PickerError picker error act node=failure />
            </List>
        </BackHandler>
    }
}

fn fit(picker: Picker, held: Memo<Option<Pick>>, menu: NodeRef, failure: NodeRef) {
    let shown = create_memo(move || {
        held.with(|held| match held {
            Some(held) if held.error.is_some() => Fit::Node(failure),
            Some(held) if held.surface_open() => Fit::Size(Vec2::new(
                SURFACE_SIZE.x,
                card_height(held),
            )),
            Some(held) if held.menu_open() => Fit::Node(menu),
            Some(_) | None => Fit::Size(Vec2::new(1.0, 1.0)),
        })
    });
    let editor = picker.editor();
    create_effect(move || {
        let shown = shown.get();
        untrack(|| match shown {
            Fit::Node(node) => editor.fit_to(Some(node)),
            Fit::Size(size) => {
                editor.fit_to(None);
                editor.set_intrinsic_size(Some(size));
            }
        });
    });
}

#[derive(Clone, Copy, PartialEq)]
enum Fit {
    Node(NodeRef),
    Size(Vec2),
}

fn card_height(held: &Pick) -> f32 {
    match held.creating.is_none() {
        true => SURFACE_SIZE.y,
        false => creation_height(Some(held)) + 2.0 * CREATION_INSET.px(),
    }
}

fn close_when_left(picker: Picker, held: Memo<Option<Pick>>) {
    let focused = with_document(|document| document.watch_focused());
    let had = std::cell::Cell::new(false);
    create_effect(move || {
        let now = focused.get().is_some();
        let left = had.replace(now) && !now;
        if left && untrack(|| held.with(|held| held.as_ref().is_some_and(Pick::menu_open))) {
            untrack(|| picker.act(PickAction::Close));
        }
    });
}

fn act_on(picker: Picker) -> Act {
    Callback::new(move |action: PickAction| picker.act(action))
}

fn close_surface(picker: Picker) {
    let creating = untrack(|| picker.pick()).and_then(|held| held.creating);
    match creating {
        Some(Creating {
            child: Some(child), ..
        }) => picker.host().dismiss_child(child),
        Some(_) => picker.act(PickAction::CancelCreation),
        None => picker.act(PickAction::Close),
    }
}

#[component]
fn PickerMenu(
    picker: Picker,
    held: Memo<Option<Pick>>,
    act: Act,
    templates: bool,
    placing: bool,
    overlay: NodeRef,
) -> NodeId {
    let open = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::menu_open)));
    let sheet = picker.sheet;
    let items = match templates {
        true => view! {
            <SlideItems picker held act />
        },
        false => view! {
            <TypeItems picker held act placing />
        },
    };
    view! {
        <MenuPopup
            items
            anchor={OverlayAnchor::Rect(Rect::ZERO)}
            placement=Placement::BelowStart
            open
            sheet
            overlay
            min_width=MENU_WIDTH
            on_dismiss={move || act.call(PickAction::Close)}
        />
    }
}

#[component]
fn PickerCard(picker: Picker, held: Memo<Option<Pick>>) -> NodeId {
    let open = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::surface_open)));
    if picker.sheet {
        return view! {
            <CentredSurface picker held />
        };
    }
    view! {
        <Overlay
            anchor={OverlayAnchor::Point(Pos2::ZERO)}
            placement=Placement::Fill
            policy=LayerPolicy::BlocksAll
            open
            on_dismiss={move |why: Dismiss| {
                if matches!(why, Dismiss::Escape | Dismiss::Back) {
                    close_surface(picker);
                }
            }}
        >
            <Frame fill=ColorRole::SurfaceRaised border={Border::DEFAULT} radius=Radius::Lg>
                <PickerSurface picker />
            </Frame>
        </Overlay>
    }
}

#[component]
fn TypeItems(
    picker: Picker,
    held: Memo<Option<Pick>>,
    act: Act,
    placing: bool,
) -> Children<MenuItem> {
    let common = create_memo(move || {
        let types = picker.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| common(&types, held))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || common.get().iter().map(tile_key).collect::<Vec<_>>());
    let templated = create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .is_some_and(|held| !picker.templates_for(held.import_parent()).is_empty())
        })
    });
    let first = create_memo(move || keys.get().first().cloned());
    let row = move |key: String| {
        let entry = create_memo({
            let key = key.clone();
            move || {
                common
                    .get()
                    .into_iter()
                    .find(|entry| tile_key(entry) == key)
            }
        });
        let separated = create_memo({
            let key = key.clone();
            move || placing && first.get().as_ref() == Some(&key)
        });
        view! {
            <TypeItem act key entry separated />
        }
    };
    match placing {
        true => view! {
            <LocationItem picker held act />
            <ForEach keys>|key: String| { row(key) }</ForEach>
            <MoreItem act label="More blocks…" />
            <Show condition={templated}>
                <TemplatesItem picker held act />
            </Show>
            <LinkItem act />
            <ImportItem held act />
        },
        false => view! {
            <ForEach keys>|key: String| { row(key) }</ForEach>
            <MoreItem act label="More blocks…" />
            <Show condition={templated}>
                <TemplatesItem picker held act />
            </Show>
            <LinkItem act />
            <ImportItem held act />
        },
    }
}

#[component]
fn MoreItem(act: Act, label: &'static str) -> MenuItem {
    view! {
        <MenuItem
            label
            glyph=ICON_BLOCKS
            separated=true
            then=MenuThen::Stay
            row_test_id="picker.more"
            on_click={move || act.call(PickAction::Stage(Stage::More))}
        />
    }
}

#[component]
fn SlideItems(picker: Picker, held: Memo<Option<Pick>>, act: Act) -> Children<MenuItem> {
    let slides = create_memo(move || {
        let types = picker.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| slides(&types, held))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || slides.get().iter().map(tile_key).collect::<Vec<_>>());
    let caption = held.get_untracked().map_or_else(String::new, |held| {
        format!("New {}", held.filter.name.to_lowercase())
    });
    view! {
        <MenuCaption label={caption} />
        <MenuGrid columns=2 row_test_id="picker.slides">
            <ForEach keys>|key: String| {
                let entry = create_memo({ let key = key.clone(); move || {
                    slides.get().into_iter().find(|entry| tile_key(entry) == key)
                } });
                view! {
                    <TypeItem act key entry separated={create_memo(|| false)} />
                }
            }</ForEach>
        </MenuGrid>
        <MoreItem act label="Other blocks…" />
        <LinkItem act />
    }
}

#[component]
fn TypeItem(
    act: Act,
    key: String,
    entry: Memo<Option<TemplateEntry>>,
    separated: Memo<bool>,
) -> MenuItem {
    let glyph_color = create_memo(move || {
        entry
            .get()
            .and_then(|entry| entry.color)
            .map(|[r, g, b]| Color32::from_rgb(r, g, b))
    });
    let label = create_memo(move || entry.get().map(|entry| entry.name).unwrap_or_default());
    let glyph = create_memo(move || entry.get().and_then(|entry| entry.icon));
    view! {
        <MenuItem
            label
            glyph
            glyph_color
            separated
            then=MenuThen::Stay
            row_test_id={format!("picker.menu.{key}")}
            on_click={move || {
                if let Some(entry) = entry.get_untracked() {
                    act.call(PickAction::Make(entry));
                }
            }}
        />
    }
}

#[component]
fn TemplatesItem(picker: Picker, held: Memo<Option<Pick>>, act: Act) -> MenuItem {
    let templates = create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .map(|held| picker.templates_for(held.import_parent()))
                .unwrap_or_default()
        })
    });
    let disabled = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.importing.is_some()))
    });
    view! {
        <MenuItem
            label="New from template"
            glyph=ICON_LAYOUT_TEMPLATE
            page=true
            disabled
            row_test_id="picker.templates"
        >
            <ForEach keys={templates}>|template: Uuid| {
                let label = create_memo(move || {
                    let types = picker.types();
                    picker
                        .info(template)
                        .map(|info| info.label(types.as_ref()).name)
                        .unwrap_or_else(|| "Template".to_owned())
                });
                view! {
                    <MenuItem
                        label
                        then=MenuThen::Stay
                        row_test_id={format!("picker.template.{template}")}
                        on_click={move || act.call(PickAction::CopyTemplate(template))}
                    />
                }
            }</ForEach>
        </MenuItem>
    }
}

#[component]
fn LinkItem(act: Act) -> MenuItem {
    view! {
        <MenuItem
            label="Link existing…"
            glyph=ICON_LINK
            then=MenuThen::Stay
            row_test_id="picker.link-existing"
            on_click={move || act.call(PickAction::Stage(Stage::Link))}
        />
    }
}

#[component]
fn ImportItem(held: Memo<Option<Pick>>, act: Act) -> MenuItem {
    let importing = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.importing.is_some()))
    });
    let label = create_memo(move || match importing.get() {
        true => "Importing…".to_owned(),
        false => "Import a file…".to_owned(),
    });
    view! {
        <MenuItem
            label
            glyph=ICON_FILE_UP
            disabled={importing}
            then=MenuThen::Stay
            row_test_id="picker.import"
            on_click={move || act.call(PickAction::Import)}
        />
    }
}

#[component]
fn LocationItem(picker: Picker, held: Memo<Option<Pick>>, act: Act) -> MenuItem {
    let place = create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.place)));
    let label = create_memo(move || {
        let types = picker.types();
        match place.get() {
            Some(BlockParent::Block(id)) => picker
                .info(id)
                .map(|info| info.label(types.as_ref()).name)
                .unwrap_or_else(|| "Untitled".to_owned()),
            _ => "Top level".to_owned(),
        }
    });
    let glyph = create_memo(move || {
        let types = picker.types();
        place
            .get()
            .and_then(BlockParent::block)
            .and_then(|id| picker.info(id))
            .and_then(|info| label_glyph(&info.label(types.as_ref())))
            .unwrap_or(ICON_FOLDER)
            .to_owned()
    });
    let rows = create_memo(move || {
        let types = picker.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| {
                    picker.tree(PickTree::Place, |graph| place_rows(&types, held, graph))
                })
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || {
        rows.get()
            .into_iter()
            .map(|row| row.key)
            .collect::<Vec<_>>()
    });
    view! {
        <MenuItem lead="New in" label glyph page=true row_test_id="picker.location">
            <ForEach keys>|key: TreeKey| {
                let row = create_memo({ let key = key.clone(); move || {
                    rows.get().into_iter().find(|row| row.key == key)
                } });
                view! {
                    <PlaceItem act place key row />
                }
            }</ForEach>
        </MenuItem>
    }
}

#[component]
fn PlaceItem(
    act: Act,
    place: Memo<Option<BlockParent>>,
    key: TreeKey,
    row: Memo<Option<TreeRow>>,
) -> MenuItem {
    let id = key.id();
    let label = create_memo(move || row.get().map(|row| row.name).unwrap_or_default());
    let glyph = create_memo(move || row.get().and_then(|row| row.icon));
    let depth = create_memo(move || row.get().map(|row| row.depth));
    let unfolded = key.clone();
    let expanded = Binding::new(
        Prop::dynamic(move || {
            row.get()
                .filter(|row| row.expandable)
                .map(|row| row.expanded)
        }),
        move |open: Option<bool>| {
            act.call(PickAction::UnfoldPlace(
                unfolded.clone(),
                open == Some(true),
            ))
        },
    );
    let checked =
        create_memo(move || Some(id.is_some() && place.get().and_then(BlockParent::block) == id));
    let named = id.map_or_else(String::new, |id| format!("picker.place.{id}"));
    view! {
        <MenuItem
            label
            glyph
            depth
            expanded
            checked
            then=MenuThen::Back
            row_test_id={named}
            on_click={move || {
                if let Some(id) = id {
                    act.call(PickAction::Place(BlockParent::Block(id)));
                }
            }}
        />
    }
}

#[component]
fn CentredSurface(picker: Picker, held: Memo<Option<Pick>>) -> NodeId {
    let open = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::surface_open)));
    let close = move || close_surface(picker);
    view! {
        <DialogFrame
            open={open}
            width={Size::Custom(SURFACE_SIZE.x)}
            narrow=true
            on_dismiss={close}
        >|| {
            let layout = use_context::<DialogLayout>();
            let creating = create_memo(move || held.with(|held| held.as_ref().is_some_and(|held| held.creating.is_some())));
            let height = create_memo(move || match layout.is_some_and(|layout| layout.sheet.get()) || creating.get() {
                true => Length::INTRINSIC,
                false => Length::Size(DIALOG_HEIGHT),
            });
            view! {
                <Frame height>
                    <PickerSurface picker />
                </Frame>
            }
        }</DialogFrame>
    }
}

#[component]
fn PickerSurface(picker: Picker) -> NodeId {
    let held = picker.held();
    let act = act_on(picker);
    let shown = create_memo(move || {
        held.with(|held| match held.as_ref() {
            Some(held)
                if held
                    .creating
                    .as_ref()
                    .is_some_and(|creating| creating.template.dialog) =>
            {
                Some(None)
            }
            Some(held) => Some(Some(held.stage)),
            None => None,
        })
    });
    view! {
        <List spacing=Space::None>
            <Dynamic value={shown}>|shown: Option<Option<Stage>>| {
                match shown {
                    Some(None) => view! {
                        <CreateBody @sizing=ItemSize::Intrinsic picker held />
                    },
                    Some(Some(Stage::Link)) => view! {
                        <LinkBody @sizing=ItemSize::Percent(100.0) picker held act />
                    },
                    Some(Some(Stage::More | Stage::Menu)) | None => view! {
                        <MoreBody @sizing=ItemSize::Percent(100.0) picker held act />
                    },
                }
            }</Dynamic>
        </List>
    }
}

fn target_name(picker: Picker, held: &Pick) -> Option<String> {
    let block = held
        .place
        .and_then(BlockParent::block)
        .or_else(|| held.parent.block())?;
    let types = picker.types();
    picker
        .info(block)
        .map(|info| info.label(types.as_ref()).name)
}

#[component]
fn SurfaceHeader(
    title: Memo<String>,
    search: Memo<String>,
    placeholder: &'static str,
    act: Act,
) -> NodeId {
    let field = NodeRef::new();
    let built = view! {
        <List spacing=HEADER_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=HEADER_SPACING>
                <Button
                    icon_only=true
                    @test_id={"picker.back"}
                    glyph=ICON_ARROW_LEFT
                    label="Back to the menu"
                    variant=ButtonVariant::Ghost
                    on_click={move || act.call(PickAction::Stage(Stage::Menu))}
                />
                <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::TITLE>{title}</Text>
            </List>
            <TextInput
                @node_ref={&field}
                value={Binding::new(search, move |value: String| act.call(PickAction::Search(value)))}
                placeholder
                label="Search"
                glyph=ICON_SEARCH
                @test_id={"picker.search"}
            />
        </List>
    };
    if let Some(node) = field.try_get() {
        with_document(|document| document.focus_into(node));
    }
    built
}

fn search_of(held: Memo<Option<Pick>>) -> Memo<String> {
    create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .map(|held| held.search.clone())
                .unwrap_or_default()
        })
    })
}

#[component]
fn MoreBody(picker: Picker, held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let title = create_memo(move || {
        held.with(|held| held.as_ref().and_then(|held| target_name(picker, held)))
            .map_or_else(
                || "Add a block".to_owned(),
                |name| format!("Add a block to {name}"),
            )
    });
    let groups = create_memo(move || {
        let types = picker.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| groups(&types, held))
                .unwrap_or_default()
        })
    });
    let titles = create_memo(move || {
        groups
            .get()
            .into_iter()
            .map(|group| group.title)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || titles.get().is_empty());
    view! {
        <DialogBody
            id="picker"
            scrolls=false
            actions={vec![DialogAction::cancel("Cancel")]}
            on_dismiss={move || act.call(PickAction::Close)}
        >
            <SurfaceHeader title search={search_of(held)} placeholder="Search block types" act />
            <Scroll @sizing=ItemSize::Percent(100.0) bleed=SIDE_BLEED>
                <List spacing=ROW_SPACING>
                    <Show condition={empty}>
                        <Text style=TextStyle::CAPTION>"No block types match."</Text>
                    </Show>
                    <ForEach keys={titles}>|title: String| {
                        let group = create_memo({ let title = title.clone(); move || {
                            groups.get().into_iter().find(|group| group.title == title)
                        } });
                        view! {
                            <TypeGroupView act group />
                        }
                    }</ForEach>
                </List>
            </Scroll>
        </DialogBody>
    }
}

#[component]
fn TypeGroupView(act: Act, group: Memo<Option<TypeGroup>>) -> NodeId {
    let title = create_memo(move || group.get().map(|group| group.title).unwrap_or_default());
    let types = create_memo(move || group.get().map(|group| group.types).unwrap_or_default());
    let keys = create_memo(move || types.get().iter().map(tile_key).collect::<Vec<_>>());
    view! {
        <List spacing=GROUP_SPACING>
            <Text style=TextStyle::CAPTION>{title}</Text>
            <ForEach keys>|key: String| {
                let entry = create_memo({ let key = key.clone(); move || {
                    types.get().into_iter().find(|entry| tile_key(entry) == key)
                } });
                view! {
                    <TypeRow act key entry />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn TypeRow(act: Act, key: String, entry: Memo<Option<TemplateEntry>>) -> NodeId {
    let glyph_color = create_memo(move || {
        let color = entry.get().and_then(|entry| entry.color);
        Some(color.map_or(TextColor::Muted, |[r, g, b]| {
            TextColor::Custom(Color32::from_rgb(r, g, b))
        }))
    });
    let label = create_memo(move || entry.get().map(|entry| entry.name).unwrap_or_default());
    let glyph = create_memo(move || entry.get().and_then(|entry| entry.icon));
    view! {
        <ListRow
            @test_id={format!("picker.more.{key}")}
            label
            glyph
            glyph_color
            size=RowSize::Regular
            on_click={move || {
                if let Some(entry) = entry.get_untracked() {
                    act.call(PickAction::Make(entry));
                }
            }}
        />
    }
}

#[component]
fn LinkBody(picker: Picker, held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let title = create_memo(move || {
        held.with(|held| held.as_ref().and_then(|held| target_name(picker, held)))
            .map_or_else(
                || "Link a block".to_owned(),
                |name| format!("Link a block into {name}"),
            )
    });
    let rows = create_memo(move || {
        let types = picker.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| picker.tree(PickTree::Link, |graph| link_rows(&types, held, graph)))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || {
        rows.get()
            .into_iter()
            .map(|row| row.key)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || keys.get().is_empty());
    let selected = create_memo(move || {
        let key = held.with(|held| held.as_ref().and_then(|held| held.selected.clone()))?;
        rows.with(|rows| rows.iter().find(|row| row.key == key).cloned())
    });
    let chosen = create_memo(move || selected.get().filter(|row| row.enabled));
    let unready = create_memo(move || chosen.get().is_none());
    let above = create_memo(move || selected.get().map(|row| row.above).unwrap_or_default());
    let has_above = create_memo(move || !above.get().is_empty());
    let name = create_memo(move || selected.get().map(|row| row.name).unwrap_or_default());
    let link = move |row: TreeRow| {
        if let (true, Some(id)) = (row.enabled, row.key.id()) {
            act.call(PickAction::Link(id, row.block_type));
        }
    };
    let item = move |key: TreeKey| {
        rows.with(|rows| {
            rows.iter().find(|row| row.key == key).map(|row| TreeItem {
                label: row.name.clone(),
                depth: row.depth,
                expandable: row.expandable,
                expanded: row.expanded,
            })
        })
        .unwrap_or(TreeItem {
            label: String::new(),
            depth: 0,
            expandable: false,
            expanded: false,
        })
    };
    let selected_key =
        create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.selected.clone())));
    let empty_text = create_memo(move || {
        match held.with(|held| {
            held.as_ref()
                .is_none_or(|held| held.search.trim().is_empty())
        }) {
            true => "No blocks are available to link.".to_owned(),
            false => "No matching blocks.".to_owned(),
        }
    });
    let named = create_memo(move || !name.get().is_empty());
    view! {
        <DialogBody
            id="picker"
            scrolls=false
            actions={vec![
                DialogAction::cancel("Cancel"),
                DialogAction::primary("Link", move || {
                    if let Some(row) = chosen.get_untracked() {
                        link(row);
                    }
                })
                .disabled(unready),
            ]}
            on_dismiss={move || act.call(PickAction::Close)}
        >
            <SurfaceHeader
                title
                search={search_of(held)}
                placeholder="Search by name or UUID"
                act
            />
            <Show condition={empty}>
                <Text style=TextStyle::CAPTION>{empty_text}</Text>
            </Show>
            <Tree
                @sizing=ItemSize::Percent(100.0)
                keys
                item
                selected={selected_key}
                spacing=Space::Xs2
                bleed=SIDE_BLEED
                row_test_id={move |key: TreeKey| match key.id() {
                    Some(id) => format!("picker.link.{id}"),
                    None => "picker.link.shared".to_owned(),
                }}
                on_select={move |key: TreeKey| act.call(PickAction::Select(key))}
                on_open={move |key: TreeKey| {
                    if let Some(row) = rows.with_untracked(|rows| rows.iter().find(|row| row.key == key).cloned()) {
                        link(row);
                    }
                }}
                on_expand={move |(key, open): (TreeKey, bool)| act.call(PickAction::Unfold(key, open))}
            >|face: TreeRowFace<TreeKey>| {
                let key = face.key.clone();
                let row = create_memo(move || rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()));
                view! {
                    <LinkRowFace row />
                }
            }</Tree>
            <Show condition={named}>
                <List direction=Direction::Horizontal align=Align::Center spacing=HEADER_SPACING>
                    <Show condition={has_above}>
                        <Text size=TextSize::Sm color=TextColor::Muted overflow=Overflow::Ellipsis>
                            {above}
                        </Text>
                        <Text size=TextSize::Sm color=TextColor::Muted>
                            <Icon glyph=ICON_CHEVRON_RIGHT />
                        </Text>
                    </Show>
                    <Text size=TextSize::Sm overflow=Overflow::Ellipsis>{name}</Text>
                </List>
            </Show>
        </DialogBody>
    }
}

#[component]
fn LinkRowFace(row: Memo<Option<TreeRow>>) -> NodeId {
    let glyph = create_memo(move || row.get().and_then(|row| row.icon));
    let label = create_memo(move || row.get().map(|row| row.name).unwrap_or_default());
    let detail = create_memo(move || row.get().map(|row| row.detail).unwrap_or_default());
    let section = create_memo(move || row.get().is_some_and(|row| row.section));
    let color =
        create_memo(
            move || match row.get().is_some_and(|row| row.enabled || row.section) {
                true => TextColor::Default,
                false => TextColor::Muted,
            },
        );
    let weight = create_memo(move || match section.get() {
        true => Weight::Bold,
        false => Weight::Normal,
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
            <Text color=TextColor::Muted size=TextSize::Sm align=TextAlign::Center>
                <Icon glyph={glyph} />
            </Text>
            <Text @sizing=ItemSize::Percent(100.0) color weight overflow=Overflow::Ellipsis>
                {label}
            </Text>
            <Text size=TextSize::Sm color=TextColor::Muted>{detail}</Text>
        </List>
    }
}

#[component]
fn CreationChild(picker: Picker, held: Memo<Option<Pick>>) -> NodeId {
    let placed = create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .and_then(|held| held.creating.as_ref())
                .filter(|creating| !creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = picker.editor();
    view! {
        <Frame width=Size::Custom(0.0) height=Size::Custom(0.0)>
            <Frame width=Size::Custom(1.0) height=Size::Custom(1.0)>
                <Subregion
                    editor={editor}
                    placed={placed}
                    mode=ChildMode::Live
                    punch=false
                    on_state={move |state: ChildState| picker.creation_state(&state)}
                />
            </Frame>
        </Frame>
    }
}

fn creation_content(creating: &Creating) -> SubregionContent {
    SubregionContent::Creation {
        editor: creating.template.editor,
        template: creating.template.template.clone(),
    }
}

#[component]
fn CreateBody(picker: Picker, held: Memo<Option<Pick>>) -> NodeId {
    let creating =
        create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.creating.clone())));
    let height =
        create_memo(move || Length::Size(Size::Custom(creation_height(held.get().as_ref()))));
    let placed = create_memo(move || {
        creating.with(|creating| {
            creating
                .as_ref()
                .filter(|creating| creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = picker.editor();
    view! {
        <Frame padding=CREATION_INSET>
            <Frame height=height>
                <Subregion
                    editor={editor}
                    placed={placed}
                    mode=ChildMode::Live
                    dialog=true
                    @test_id={"picker.creation"}
                    on_state={move |next: ChildState| picker.creation_state(&next)}
                />
            </Frame>
        </Frame>
    }
}

fn creation_height(held: Option<&Pick>) -> f32 {
    held.and_then(|held| held.creating.as_ref())
        .and_then(|creating| creating.height)
        .unwrap_or(CREATION_HEIGHT)
}

#[component]
fn PickerError(picker: Picker, error: Memo<Option<String>>, act: Act, node: NodeRef) -> NodeId {
    let open = create_memo(move || error.get().is_some());
    let text = create_memo(move || error.get().unwrap_or_default());
    if picker.sheet {
        return view! {
            <Dialog
                open={open}
                title="Block picker error"
                width=Size::Popover
                kind=DialogKind::Alert
                actions={vec![DialogAction::cancel("Dismiss")]}
                on_dismiss={move || act.call(PickAction::DismissError)}
            >
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{text}</Text>
            </Dialog>
        };
    }
    view! {
        <Overlay
            anchor={OverlayAnchor::Point(Pos2::ZERO)}
            placement=Placement::At
            policy=LayerPolicy::BlocksAll
            open
            on_dismiss={move |why: Dismiss| {
                if matches!(why, Dismiss::Escape | Dismiss::Back) {
                    act.call(PickAction::DismissError);
                }
            }}
        >
            <Frame
                @node_ref={&node}
                width={Size::Popover}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Lg
            >
                <DialogBody
                    kind=DialogKind::Alert
                    title="Block picker error"
                    actions={vec![DialogAction::cancel("Dismiss")]}
                    on_dismiss={move || act.call(PickAction::DismissError)}
                >
                    <Text style=TextStyle::CAPTION color=TextColor::Danger>{text}</Text>
                </DialogBody>
            </Frame>
        </Overlay>
    }
}
