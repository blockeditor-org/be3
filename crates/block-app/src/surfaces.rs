use std::{cell::RefCell, collections::HashMap, rc::Rc};

use beui::reactive::{
    ForEach, Frame, Layers, List, Memo, ReadSignal, Show, Text, WriteSignal, clone, component,
    create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Caption, Heading, Icon, Spinner, use_theme};
use beui::{Align, Color32, NodeId, TextAlign};
use block_plugin_api::{EditorInstanceId, EditorRegion, FrameSpec};

use crate::compositor::{ChildView, PaneSurface, PluginRegion, RegionEditor, ShellSurface};
use crate::host::{self, HostCommand, HostItem};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SurfaceId {
    Creation,
    NestedCreation,
    ArtifactSettings,
}

const HOSTING: [SurfaceId; 3] = [
    SurfaceId::Creation,
    SurfaceId::NestedCreation,
    SurfaceId::ArtifactSettings,
];

#[derive(Clone)]
pub(crate) struct HostedRegion {
    pub(crate) editor: RegionEditor,
    pub(crate) region: EditorRegion,
    pub(crate) frame: Option<FrameSpec>,
}

impl HostedRegion {
    fn key(&self) -> (EditorInstanceId, EditorRegion) {
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
    region: ReadSignal<Option<HostedRegion>>,
    set_region: WriteSignal<Option<HostedRegion>>,
    height: ReadSignal<Option<f32>>,
    set_height: WriteSignal<Option<f32>>,
}

#[derive(Default)]
struct Declared {
    regions: HashMap<SurfaceId, HostedRegion>,
    heights: HashMap<SurfaceId, f32>,
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
            let (region, set_region) = create_signal(None);
            let (height, set_height) = create_signal(None);
            hosted.insert(
                id,
                Hosted {
                    region,
                    set_region,
                    height,
                    set_height,
                },
            );
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

pub(crate) fn set_height(id: SurfaceId, height: Option<f32>) {
    DECLARED.with(|declared| {
        let mut declared = declared.borrow_mut();
        match height {
            Some(height) => declared.heights.insert(id, height),
            None => declared.heights.remove(&id),
        };
    });
}

pub(crate) fn height(id: SurfaceId) -> ReadSignal<Option<f32>> {
    hosted(id).height
}

pub(crate) fn apply() {
    let declared = DECLARED.with(|declared| std::mem::take(&mut *declared.borrow_mut()));
    for id in HOSTING {
        let hosted = hosted(id);
        let region = declared.regions.get(&id).cloned();
        if hosted.region.with_untracked(|held| *held != region) {
            hosted.set_region.set(region);
        }
        let height = declared.heights.get(&id).copied();
        if hosted.height.get_untracked() != height {
            hosted.set_height.set(height);
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
pub(crate) fn PluginPane(pane: u64) -> NodeId {
    let pane = block_plugin_api::PaneId(pane);
    let shell = crate::compositor::shell();
    let shell = create_memo(move || shell.get());
    view! {
        <PaneSurface shell pane />
    }
}

#[component]
pub(crate) fn HostedSurface(id: SurfaceId) -> NodeId {
    let region = hosted(id).region;
    let keys = create_memo(clone!(region -> move || {
        region.with(|held| held.as_ref().map(HostedRegion::key).into_iter().collect::<Vec<_>>())
    }));
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |_key: (EditorInstanceId, EditorRegion)| {
                    let Some(hosted) = region.get_untracked() else {
                        return view! {
                            <Frame />
                        };
                    };
                    let child_view: ChildView = Rc::new(|_, _, _| view! {
                        <Frame />
                    });
                    view! {
                        <PluginRegion
                            editor={hosted.editor}
                            region={hosted.region}
                            frame={hosted.frame}
                            child_view
                        />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[component]
pub(crate) fn HostItemFace(content: Memo<HostItem>) -> NodeId {
    let kind = content.get_untracked();
    let text = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Notice { text, .. } | HostItem::Error { text, .. } => text,
            HostItem::Fallback { name, .. } => name,
            HostItem::Unsupported { block, .. } => format!("Block: {block}"),
        }
    });
    let detail = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Unsupported { block_type, .. } => format!("Type: {block_type}"),
            _ => String::new(),
        }
    });
    let italic = create_memo({
        let content = content.clone();
        move || {
            matches!(
                content.get(),
                HostItem::Fallback {
                    automatic: true,
                    ..
                }
            )
        }
    });
    let spinning = create_memo({
        let content = content.clone();
        move || matches!(content.get(), HostItem::Notice { spinner: true, .. })
    });
    let restart = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Error { restart, .. } => restart,
            _ => None,
        }
    });
    let restartable = create_memo({
        let restart = restart.clone();
        move || restart.get().is_some()
    });
    let glyph = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Fallback { icon, .. } => icon.unwrap_or_default(),
            _ => String::new(),
        }
    });
    let has_glyph = create_memo({
        let glyph = glyph.clone();
        move || !glyph.get().is_empty()
    });
    let theme = use_theme();
    let (surface, outline, color) = match kind {
        HostItem::Fallback { .. } => (
            Color32::from_gray(28),
            Color32::from_gray(75),
            Color32::from_gray(211),
        ),
        HostItem::Error { .. } => (
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            theme.danger.get_untracked(),
        ),
        _ => (
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            theme.text_muted.get_untracked(),
        ),
    };
    let unsupported = matches!(kind, HostItem::Unsupported { .. });
    view! {
        <Frame
            color=surface
            outline=outline
            outline_width=1.0
            outline_visible=true
            radius=5
            padding_horizontal=12.0
            padding_vertical=12.0
        >
            <List spacing=8.0 align=Align::Center>
                <Show condition={unsupported}>
                    <Heading content="Unsupported block type" align=TextAlign::Center />
                </Show>
                <Show condition={has_glyph}>
                    <Icon glyph={glyph} text_size=28.0 color=color />
                </Show>
                <Text string={text} font_size=14.0 align=TextAlign::Center color=color italic />
                <Show condition={unsupported}>
                    <Caption content={detail} align=TextAlign::Center />
                </Show>
                <Show condition={spinning}>
                    <Spinner />
                </Show>
                <Show condition={restartable}>
                    <Button
                        label="Restart plugin"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            if let Some(plugin) = restart.get_untracked() {
                                host::push_command(HostCommand::RestartPlugin(plugin));
                            }
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}
