use std::{cell::RefCell, collections::HashMap};

use beui::reactive::*;
use beui::styled::*;
use beui::{Align, NodeId};
use block_plugin_api::{
    BlockCommand, BlockLocation, DialogFrame, EditorInstanceId, EditorRegion, FrameSpec,
};
use uuid::Uuid;

use crate::compositor::{ChildView, PluginRegion, RegionEditor, ShellSurface};
use beui::{Align2, Direction};

mod notices;

pub(crate) use notices::{CrashNotice, StaleNotice};


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SurfaceId {
    ArtifactSettings,
}

const HOSTING: [SurfaceId; 1] = [SurfaceId::ArtifactSettings];

#[derive(Clone)]
pub(crate) struct HostedRegion {
    pub(crate) editor: RegionEditor,
    pub(crate) region: EditorRegion,
    pub(crate) frame: Option<FrameSpec>,
}

impl HostedRegion {
    pub(crate) fn key(&self) -> (EditorInstanceId, EditorRegion) {
        (self.editor.instance, self.region)
    }
}

impl PartialEq for HostedRegion {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key() && self.frame == other.frame
    }
}

#[derive(Clone)]
struct Hosted {
    region: Signal<Option<HostedRegion>>,
}

#[derive(Default)]
struct Declared {
    regions: HashMap<SurfaceId, HostedRegion>,
}

thread_local! {
    static HOSTED: RefCell<HashMap<SurfaceId, Hosted>> = RefCell::new(HashMap::new());
    static DECLARED: RefCell<Declared> = RefCell::new(Declared::default());
}

pub(crate) fn create_handles() {
    HOSTED.with(|hosted| {
        let mut hosted = hosted.borrow_mut();
        hosted.clear();
        for id in HOSTING {
            let region = create_signal(None);
            hosted.insert(id, Hosted { region });
        }
    });
}

fn hosted(id: SurfaceId) -> Hosted {
    HOSTED.with(|hosted| {
        hosted
            .borrow()
            .get(&id)
            .cloned()
            .expect("the hosting surfaces are created while the document is built")
    })
}

pub(crate) fn host(id: SurfaceId, region: Option<HostedRegion>) {
    DECLARED.with(|declared| {
        let mut declared = declared.borrow_mut();
        match region {
            Some(region) => declared.regions.insert(id, region),
            None => declared.regions.remove(&id),
        };
    });
}

pub(crate) fn apply() {
    let declared = DECLARED.with(|declared| std::mem::take(&mut *declared.borrow_mut()));
    for id in HOSTING {
        let hosted = hosted(id);
        let region = declared.regions.get(&id).cloned();
        if hosted.region.with_untracked(|held| *held != region) {
            hosted.region.set(region);
        }
    }
}

#[component]
pub(crate) fn MainSurface() -> NodeId {
    let shell = crate::compositor::shell();
    let shell = create_memo(move || shell.get());
    view! {
        <ShellSurface shell />
    }
}

#[component]
pub(crate) fn HostedSurface(
    id: SurfaceId,
    #[prop(default = None)] dialog: Prop<Option<DialogFrame>>,
) -> NodeId {
    let region = hosted(id).region;
    let keys = create_memo(move || {
        region.with(|held| {
            held.as_ref()
                .map(HostedRegion::key)
                .into_iter()
                .collect::<Vec<_>>()
        })
    });
    view! {
        <Layers>
            <ForEach keys={keys}>|_key: (EditorInstanceId, EditorRegion)| {
                let Some(hosted) = region.get_untracked() else {
                    return view! {
                        <Frame />
                    };
                };
                let child_view: ChildView = store(Box::new(|_, _, _| view! {
                    <Frame />
                }));
                let frame = hosted.frame;
                let frame = Prop::dynamic(move || match dialog.get() {
                    Some(dialog) => Some(FrameSpec {
                        dialog: Some(dialog),
                        ..frame.clone().unwrap_or_default()
                    }),
                    None => frame.clone(),
                });
                view! {
                    <PluginRegion editor={hosted.editor} region={hosted.region} frame child_view />
                }
            }</ForEach>
        </Layers>
    }
}


#[component]
pub(crate) fn StaleBlock(block: Uuid, block_type: Uuid, stale: block_app_stack::Stale) -> NodeId {
    let can_try = matches!(stale, block_app_stack::Stale::Reshaped);
    let detail = match stale {
        block_app_stack::Stale::Unreadable(error) => {
            format!("This build cannot read what another build saved here: {error}")
        }
        block_app_stack::Stale::Reshaped => {
            "Another build saved this block in a shape this build may read wrongly.".to_owned()
        }
    };
    let delete = move || {
        let source = block_app_stack::node(block).and_then(|node| node.parent).map_or(
            BlockLocation::Detached,
            |parent| match parent {
                be_graph::BlockParent::Root => BlockLocation::Root,
                be_graph::BlockParent::Detached => BlockLocation::Detached,
                be_graph::BlockParent::Block(parent) => BlockLocation::Block(parent.into_bytes()),
            },
        );
        crate::compositor::act(
            None,
            block_app_plugin_host::EditorAction::Command {
                id: block,
                command: BlockCommand::Delete {
                    block_type: block_type.into_bytes(),
                    source,
                },
            },
        );
    };
    view! {
        <Frame align=Align::Center>
            <StaleNotice
                title="Saved by another build"
                detail={detail}
                can_try
                on_reset={move || block_app_stack::reset(block)}
                on_delete={delete}
                on_try={move || block_app_stack::try_anyway(block)}
            />
        </Frame>
    }
}

#[component]
pub(crate) fn ConflictNotice(block: Uuid, count: usize) -> NodeId {
    let text = match count {
        1 => "Merged with a conflicting change from another device".to_owned(),
        count => format!("Merged with {count} conflicting changes from another device"),
    };
    view! {
        <Frame align={Align2::new(Align::Center, Align::Start)} padding={Padding::all(Space::Md)}>
            <Frame
                @test_id={format!("conflict.{block}")}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Md
                padding={Padding::symmetric(Space::Lg, Space::Sm)}
            >
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    spacing=Space::Md
                    wrap=true
                >
                    <Text color=TextColor::Muted>
                        <Icon glyph=beui::icons::ICON_GIT_MERGE_CONFLICT />
                    </Text>
                    <Text size=TextSize::Sm>{text}</Text>
                    <Button
                        @test_id={format!("conflict.{block}.compare")}
                        label="See both versions"
                        variant=ButtonVariant::Secondary
                        on_click={move || block_app_stack::conflicts::compare(block)}
                    />
                    <Button
                        @test_id={format!("conflict.{block}.dismiss")}
                        label="Keep merged"
                        variant=ButtonVariant::Ghost
                        on_click={move || block_app_stack::conflicts::dismiss(block)}
                    />
                </List>
            </Frame>
        </Frame>
    }
}
