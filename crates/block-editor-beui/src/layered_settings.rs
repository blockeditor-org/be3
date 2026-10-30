use be_block::be_model::Document;
use be_block::{
    BlockContent, Layered, Layers, LiveEdit, Resolution, Setting, SettingsLayer, SettingsSource,
};
use beui::NodeId;
use beui::icons::{ICON_PIN, ICON_PIN_OFF, ICON_ROTATE_CCW};
use beui::reactive::*;
use beui::styled::*;
use uuid::Uuid;

use crate::root_settings::{SettingsGraph, own_settings_blocks};
use crate::{
    Editor, SettingsAnswer, SettingsChangeLog, SettingsLayers, SettingsTarget as Target,
    Subscription,
};

const KEPT: &str = "Kept on this device";
const IN_PROFILE: &str = "Kept in this profile";
const ONLY_HERE: &str = "This device only";
const DEVELOPER_OVERRIDE: &str = "Overridden by developer tools";
const ORGANIZATION: &str = "Set by your organization";
const KEEP_ACTION: &str = "keep";
const UNDO_ACTION: &str = "undo";

struct ChangeLog {
    log: SettingsChangeLog,
    _changed: Subscription,
}

pub struct LayeredSettings<R: 'static> {
    editor: Editor,
    target: Memo<Target>,
    layers: Memo<Layers>,
    every: Memo<R>,
    here: Memo<R>,
    own: Memo<R>,
    below: Memo<R>,
    effective: Memo<R>,
    resolved: Memo<Resolution<R>>,
    ready: Memo<bool>,
    blocks: Stored<SettingsLayers<R>>,
    changes: Stored<ChangeLog>,
    changed: Signal<u64>,
}

impl<R: 'static> Clone for LayeredSettings<R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: 'static> Copy for LayeredSettings<R> {}

fn settings_loaded(editor: &Editor) -> bool {
    let Some(blocks) = own_settings_blocks(editor) else {
        return false;
    };
    let client = editor.host().client_id();
    blocks.iter().all(|block| {
        editor.settings(*block).is_some_and(|settings| {
            settings
                .profile(client)
                .is_none_or(|profile| editor.settings_profile(profile).is_some())
        })
    })
}

impl<R> LayeredSettings<R>
where
    R: Layered + PartialEq + 'static,
    Document<R>: LiveEdit<Op = be_block::Edit> + BlockContent + Clone + Default,
{
    pub fn this_device(editor: Editor) -> Self {
        Self::new(editor, false)
    }

    pub fn page(editor: Editor) -> Self {
        Self::new(editor, true)
    }

    fn new(editor: Editor, page: bool) -> Self {
        let blocks = store(SettingsLayers::<R>::new(editor.core()));
        let layers = create_memo(move || blocks.with(|blocks| blocks.layer_blocks(&editor)));
        let block = editor.block_id();
        let target =
            create_memo(move || layers.with(|layers| Target::choose(page, block, layers)));
        let content = move |block: Option<Uuid>| -> R {
            block
                .and_then(|block| {
                    editor
                        .content_of::<Document<R>>(block)
                        .read(|content| content.root())
                        .ok()
                })
                .unwrap_or_default()
        };
        let every = create_memo(move || content(layers.get().every_device));
        let here = create_memo(move || content(layers.get().this_device));
        let own = create_memo(move || match page {
            true => content(Some(block)),
            false => R::default(),
        });
        let below = match page {
            true => editor.inherited_settings::<R>(),
            false => create_memo(R::default),
        };
        let effective = create_memo(move || match target.get() {
            Target::ThisDevice => here.get().over(&every.get()),
            Target::Block(_) => own.get().over(&below.get()),
        });
        let ready = create_memo(move || {
            let loaded = |block: Option<Uuid>| {
                block.is_none_or(|block| {
                    editor.content_of::<Document<R>>(block).read(|_| ()).is_ok()
                })
            };
            let layers = layers.get();
            settings_loaded(&editor)
                && loaded(layers.every_device)
                && loaded(layers.this_device)
                && (!page || loaded(Some(block)))
        });
        wait_for(move || match ready.get() {
            true => Ok(()),
            false => Err(Pending::Loading),
        });
        let log = SettingsChangeLog::default();
        let changed = create_signal(0_u64);
        Self {
            editor,
            target,
            layers,
            every,
            here,
            own,
            below,
            effective,
            resolved: editor.resolved_settings::<R>(editor.block_id()),
            ready,
            blocks,
            changes: store(ChangeLog {
                _changed: log.on_change(move || changed.update(|count| *count += 1)),
                log,
            }),
            changed,
        }
    }

    pub fn effective(&self) -> Memo<R> {
        self.effective
    }

    pub fn ready(&self) -> Memo<bool> {
        self.ready
    }

    pub fn on_this_device(&self) -> Memo<bool> {
        let target = self.target;
        create_memo(move || target.get() == Target::ThisDevice)
    }

    fn layer<V: Clone>(&self, setting: &Setting<R, V>) -> Option<SettingsLayer> {
        match self.target.get_untracked() {
            Target::ThisDevice => Some(self.here.with_untracked(|here| setting.layer(here))),
            Target::Block(_) => None,
        }
    }

    pub fn value<V: Clone + PartialEq + 'static>(
        &self,
        setting: &Setting<R, V>,
    ) -> Memo<Option<V>> {
        let (target, every, here, own) = (self.target, self.every, self.here, self.own);
        let setting = setting.clone();
        create_memo(move || match target.get() {
            Target::ThisDevice => here.with(|here| match setting.kept(here) {
                true => setting.get(here),
                false => every.with(|every| setting.get(every)),
            }),
            Target::Block(_) => own.with(|own| setting.get(own)),
        })
    }

    pub fn fallback<V: Clone + PartialEq + 'static>(
        &self,
        setting: &Setting<R, V>,
    ) -> Memo<Option<V>> {
        let (target, every, here, below) = (self.target, self.every, self.here, self.below);
        let setting = setting.clone();
        create_memo(move || match target.get() {
            Target::ThisDevice => match here.with(|here| setting.kept(here)) {
                true => every.with(|every| setting.get(every)),
                false => None,
            },
            Target::Block(_) => below.with(|below| setting.get(below)),
        })
    }

    pub fn kept<V: Clone + 'static>(&self, setting: &Setting<R, V>) -> Memo<bool> {
        let (target, here) = (self.target, self.here);
        let setting = setting.clone();
        create_memo(move || {
            target.get() == Target::ThisDevice
                && !setting.is_this_device_only()
                && here.with(|here| setting.get(here).is_some())
        })
    }

    pub fn set<V: Clone + PartialEq + 'static>(&self, setting: &Setting<R, V>, value: Option<V>) {
        let Some(layer) = self.layer(setting) else {
            if let Target::Block(block) = self.target.get_untracked() {
                self.write_block(block, setting, value);
            }
            return;
        };
        if layer == SettingsLayer::EveryDevice {
            let old = self.every.with_untracked(|every| setting.get(every));
            if old == value {
                return;
            }
            self.note(setting, old);
        }
        self.write(layer, setting, value);
    }

    pub fn setter<V: Clone + PartialEq + 'static>(
        &self,
        setting: &Setting<R, V>,
    ) -> Callback<Option<V>> {
        let (this, setting) = (*self, setting.clone());
        Callback::new(move |value: Option<V>| this.set(&setting, value))
    }

    pub fn reset<V: Clone + PartialEq + 'static>(&self, setting: &Setting<R, V>) {
        self.set(setting, None);
    }

    pub fn keep<V: Clone + PartialEq + 'static>(&self, setting: &Setting<R, V>, on: bool) {
        if self.target.get_untracked() != Target::ThisDevice || setting.is_this_device_only() {
            return;
        }
        let value = match on {
            true => {
                let Some(value) = self
                    .effective
                    .with_untracked(|effective| setting.get(effective))
                    .or(setting.default_value())
                else {
                    return;
                };
                Some(value)
            }
            false => None,
        };
        self.write(SettingsLayer::ThisDevice, setting, value);
    }

    pub fn block_for<V: Clone>(&self, setting: &Setting<R, V>) -> Option<Uuid> {
        match (self.target.get_untracked(), self.layer(setting)) {
            (Target::Block(block), _) => Some(block),
            (Target::ThisDevice, Some(layer)) => self.ensure(layer),
            (Target::ThisDevice, None) => None,
        }
    }

    pub fn row<V: Clone + PartialEq + 'static>(&self, setting: &Setting<R, V>) -> SettingHandle {
        let this = *self;
        let modified = {
            let value = self.value(setting);
            create_memo(move || value.with(Option::is_some))
        };
        let effective = self.effective;
        let keepable = {
            let setting = setting.clone();
            create_memo(move || {
                effective.with(|effective| setting.get(effective)).is_some()
                    || setting.default_value().is_some()
            })
        };
        let keep = {
            let setting = setting.clone();
            Callback::new(move |on: bool| this.keep(&setting, on))
        };
        let reset = {
            let setting = setting.clone();
            Callback::new(move |()| this.reset(&setting))
        };
        let (resolved, editor) = (self.resolved, self.editor);
        let source = {
            let setting = setting.clone();
            create_memo(move || {
                let locked =
                    resolved.with(|resolved| resolved.locked_by(|layer| setting.get(layer)));
                locked.map(|source| match source {
                    SettingsSource::Block(owner) => {
                        let types = editor.block_types();
                        let name = editor.blocks().info(owner).map_or_else(
                            || "a block you can't open".to_owned(),
                            |info| info.label(types.as_ref()).name,
                        );
                        format!("Set in {name}")
                    }
                    _ => ORGANIZATION.to_owned(),
                })
            })
        };
        SettingHandle {
            source,
            device: self.target.get_untracked() == Target::ThisDevice,
            only_here: setting.is_this_device_only(),
            modified,
            kept: self.kept(setting),
            keepable,
            keep,
            reset,
        }
    }

    pub fn changes(&self) -> SettingsChanges {
        let (changes, changed) = (self.changes, self.changed);
        let toasts = create_memo(move || {
            changed.get();
            let (message, round) = changes.with(|changes| (changes.log.message(), changes.log.round()));
            match message {
                Some(message) => vec![Toast {
                    id: round,
                    message,
                    actions: vec![
                        ToastAction {
                            key: KEEP_ACTION.to_owned(),
                            label: "Keep on this device only".to_owned(),
                        },
                        ToastAction {
                            key: UNDO_ACTION.to_owned(),
                            label: "Undo".to_owned(),
                        },
                    ],
                    sticky: true,
                    ..Toast::default()
                }],
                None => Vec::new(),
            }
        });
        SettingsChanges {
            toasts,
            on_action: Callback::new(move |key: String| {
                let answer = match key.as_str() {
                    KEEP_ACTION => Some(SettingsAnswer::KeepOnThisDevice),
                    UNDO_ACTION => Some(SettingsAnswer::Undo),
                    _ => None,
                };
                changes.with(|changes| match answer {
                    Some(answer) => changes.log.answer(answer),
                    None => changes.log.dismiss(),
                });
            }),
            on_dismiss: Callback::new(move |()| changes.with(|changes| changes.log.dismiss())),
        }
    }

    fn note<V: Clone + PartialEq + 'static>(&self, setting: &Setting<R, V>, old: Option<V>) {
        let this = *self;
        let undo = {
            let (setting, old) = (setting.clone(), old.clone());
            move || this.write(SettingsLayer::EveryDevice, &setting, old.clone())
        };
        let keep = {
            let setting = setting.clone();
            move || {
                let now = this.every.with_untracked(|every| setting.get(every));
                this.write(SettingsLayer::ThisDevice, &setting, now);
                this.write(SettingsLayer::EveryDevice, &setting, old.clone());
            }
        };
        self.changes
            .with(|changes| changes.log.note(setting.key(), setting.name(), undo, keep));
    }

    fn ensure(&self, layer: SettingsLayer) -> Option<Uuid> {
        let layers = self.layers.get_untracked();
        untrack(|| self.blocks.with(|blocks| blocks.ensure(layers, layer)))
    }

    fn write<V: Clone>(&self, layer: SettingsLayer, setting: &Setting<R, V>, value: Option<V>) {
        let layers = self.layers.get_untracked();
        untrack(|| {
            self.blocks
                .with(|blocks| blocks.write(layers, layer, setting, value))
        });
    }

    fn write_block<V: Clone>(&self, block: Uuid, setting: &Setting<R, V>, value: Option<V>) {
        untrack(|| {
            self.blocks
                .with(|blocks| blocks.write_block(block, setting, value))
        });
    }
}

#[derive(Clone, Copy)]
pub struct SettingHandle {
    source: Memo<Option<String>>,
    device: bool,
    only_here: bool,
    modified: Memo<bool>,
    kept: Memo<bool>,
    keepable: Memo<bool>,
    keep: Callback<bool>,
    reset: Callback<()>,
}

impl SettingHandle {
    pub fn with_reset(self, reset: Callback<()>) -> Self {
        Self { reset, ..self }
    }

    pub fn modified(&self) -> Memo<bool> {
        self.modified
    }

    pub fn kept(&self) -> Memo<bool> {
        self.kept
    }
}

#[derive(Clone, Copy)]
pub struct SettingsChanges {
    toasts: Memo<Vec<Toast>>,
    on_action: Callback<String>,
    on_dismiss: Callback<()>,
}

#[component]
pub fn ChangesToast(changes: SettingsChanges, anchor: NodeRef) -> NodeId {
    let SettingsChanges {
        toasts,
        on_action,
        on_dismiss,
    } = changes;
    view! {
        <Toasts
            anchor=anchor
            toasts={toasts}
            on_dismiss={move |_: u64| on_dismiss.call(())}
            on_action={move |(_, key): (u64, String)| on_action.call(key)}
        />
    }
}

#[component]
pub fn LayeredSettingsRow(
    setting: SettingHandle,
    label: Prop<String>,
    #[prop(default = String::new())] description: Prop<String>,
    #[prop(default = false)] separated: Prop<bool>,
    #[prop(default = false)] compact: bool,
    #[prop(default = false)] locked: Prop<bool>,
    #[prop(default = false)] overridden: Prop<bool>,
    id: String,
    children: Children<ListChild>,
) -> NodeId {
    let SettingHandle {
        source,
        device,
        only_here,
        modified,
        kept,
        keepable,
        keep,
        reset,
    } = setting;
    let locked = create_memo(move || locked.get() || source.with(Option::is_some));
    let unresettable = create_memo(move || !modified.get() || locked.get());
    let menu: Children<MenuItem> = match device {
        true => {
            let checked = create_memo(move || Some(kept.get()));
            let unkeepable = create_memo(move || locked.get() || (!kept.get() && !keepable.get()));
            let keeping = (!only_here).then(|| {
                view! {
                    <MenuItem
                        label="Keep on this device"
                        glyph=ICON_PIN
                        checked={checked}
                        disabled={unkeepable}
                        row_test_id={format!("{id}.keep")}
                        on_click={move || keep.call(!kept.get_untracked())}
                    />
                }
            });
            let resetting = view! {
                <MenuItem
                    label="Reset to default"
                    glyph=ICON_ROTATE_CCW
                    disabled={unresettable}
                    row_test_id={format!("{id}.reset")}
                    on_click={move || reset.call(())}
                />
            };
            keeping.into_iter().chain([resetting]).collect()
        }
        false => [view! {
            <MenuItem
                label="Stop keeping in this profile"
                glyph=ICON_PIN_OFF
                disabled={unresettable}
                row_test_id={format!("{id}.reset")}
                on_click={move || reset.call(())}
            />
        }]
        .into_iter()
        .collect(),
    };
    let note = create_memo(move || {
        if overridden.get() {
            return DEVELOPER_OVERRIDE.to_owned();
        }
        if let Some(source) = source.get() {
            return source;
        }
        match (device, only_here, kept.get(), modified.get()) {
            (true, true, _, _) => ONLY_HERE.to_owned(),
            (true, false, true, _) => KEPT.to_owned(),
            (false, _, _, true) => IN_PROFILE.to_owned(),
            _ => String::new(),
        }
    });
    view! {
        <SettingsRow
            label
            description
            separated
            compact
            modified={modified}
            locked={locked}
            note={note}
            menu_test_id={format!("{id}.menu")}
            menu={menu}
        >
            {children}
        </SettingsRow>
    }
}
