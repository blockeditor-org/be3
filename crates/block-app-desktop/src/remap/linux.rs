use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use be_block::be_model::Document;
use be_block::{BlockContent, InputRemapper, InputSettings, Root};
use be_remap_host::Remapper;
use be_remap_host::be_remap::protocol::{decode, encode};
use be_remap_host::be_remap::rules::exe_name;
use be_remap_host::be_remap::{Input, Output, Source, Window};
use beui_platform_drm::{InputControl, InputFilter, RawInput};
use block_plugin_api::{RemapState, RemapsWatch};
use uuid::Uuid;

use block_app_base::app_state::AppStateStore;
use block_app_stack::discovery;
use loader::{Loader, Step, Wanted};

mod loader;

const BUTTON_FIRST: u32 = 0x100;

type Fetched = Arc<Mutex<Option<(String, String, Result<Vec<u8>, String>)>>>;

#[derive(Default)]
struct State {
    control: Option<InputControl>,
    filter: Option<Rc<RefCell<Filter>>>,
    chosen: Option<Uuid>,
    seen: Option<u64>,
    loader: Loader,
    fetched: Fetched,
    window: Option<Window>,
    locked: bool,
    save: Option<Option<Vec<u8>>>,
    module: Vec<u8>,
    published: Option<RemapState>,
}

#[derive(Default)]
struct Filter {
    remapper: Option<Remapper>,
    now: Duration,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

impl InputFilter for Filter {
    fn input(&mut self, input: RawInput, now: Duration) -> Vec<RawInput> {
        self.now = now;
        let (Some(remapper), Some(remapped)) = (self.remapper.as_mut(), remap_input(input)) else {
            return vec![input];
        };
        let outputs = remapper.input(remapped, micros(now));
        self.report();
        outputs.into_iter().map(raw).collect()
    }

    fn timer(&mut self, now: Duration) -> Vec<RawInput> {
        self.now = now;
        let Some(remapper) = self.remapper.as_mut() else {
            return Vec::new();
        };
        let outputs = remapper.timer(micros(now));
        self.report();
        outputs.into_iter().map(raw).collect()
    }

    fn deadline(&self) -> Option<Duration> {
        self.remapper
            .as_ref()
            .and_then(Remapper::deadline)
            .map(Duration::from_micros)
    }

    fn forget_held(&mut self) {
        if let Some(remapper) = self.remapper.as_mut() {
            remapper.forget_held();
        }
    }
}

impl Filter {
    fn report(&mut self) {
        let problems = self
            .remapper
            .as_mut()
            .map(Remapper::take_problems)
            .unwrap_or_default();
        for problem in problems {
            block_app_base::notices::report(problem);
        }
    }
}

pub fn start(control: Option<InputControl>) {
    let Some(control) = control else {
        return;
    };
    let filter = Rc::new(RefCell::new(Filter::default()));
    control.set_filter(Some(filter.clone()));
    STATE.with_borrow_mut(|state| {
        state.control = Some(control);
        state.filter = Some(filter);
    });
}

pub fn load_cached(cached: Option<Vec<u8>>) {
    let Some((wanted, module)) = cached
        .and_then(|cached| decode::<(String, String, Vec<u8>, Vec<u8>)>(&cached).ok())
        .map(|(plugin, module_path, module, config)| {
            (
                Wanted {
                    plugin,
                    module: module_path,
                    config,
                },
                module,
            )
        })
    else {
        return;
    };
    install(&wanted, &module);
    STATE.with_borrow_mut(|state| state.loader.installed(wanted));
}

pub fn configure(settings: &InputSettings) {
    let chosen = settings.remapper.map(|choice| choice.block);
    let cleared = STATE.with_borrow_mut(|state| {
        if state.chosen == chosen {
            return false;
        }
        state.chosen = chosen;
        state.seen = None;
        chosen.is_none()
    });
    if cleared {
        want(None);
    }
}

pub fn focus(window: Option<be_wayland::WindowInfo>) {
    let window = window.map(|window| Window {
        exe: exe_name(&window.command),
        app_id: window.app_id,
        title: window.title,
    });
    let changed = STATE.with_borrow_mut(|state| {
        if state.window == window {
            return false;
        }
        state.window.clone_from(&window);
        true
    });
    if changed {
        with_remapper(|remapper, now| remapper.focus(window, now));
    }
}

pub fn set_locked(locked: bool) {
    STATE.with_borrow_mut(|state| state.locked = locked);
    with_remapper(|remapper, _| remapper.set_locked(locked));
}

pub fn frame(store: &AppStateStore) {
    read_chosen();
    take_fetched();
    if let Some(save) = STATE.with_borrow_mut(|state| state.save.take())
        && let Err(error) = store.set_remapper(save.as_deref())
    {
        eprintln!("block-app: the input remapper was not kept for the next session: {error}");
    }
    let shown = STATE.with_borrow(|state| {
        let value = match state
            .filter
            .as_ref()
            .map(|filter| filter.borrow().remapper.as_ref().map(Remapper::stopped))
        {
            Some(Some(false)) => RemapState::On,
            Some(Some(true)) => RemapState::Stopped,
            _ => RemapState::Off,
        };
        (state.published != Some(value)).then_some(value)
    });
    if let Some(value) = shown {
        block_app_base::watches::publish::<RemapsWatch>(&value);
        STATE.with_borrow_mut(|state| state.published = Some(value));
    }
}

fn read_chosen() {
    let Some(block) = STATE.with_borrow(|state| state.chosen) else {
        return;
    };
    block_app_stack::hold(block, InputRemapper::CONTENT_TYPE);
    let Some(revision) = block_app_stack::content_revision(block) else {
        return;
    };
    if STATE.with_borrow(|state| state.seen == Some(revision)) {
        return;
    }
    let Some(content) = block_app_stack::content(block) else {
        return;
    };
    STATE.with_borrow_mut(|state| state.seen = Some(revision));
    let wanted = Document::<InputRemapper>::decode(content.bytes())
        .ok()
        .and_then(|content| {
            let remapper = content.root();
            let (plugin, module, config) = remapper.runs()?;
            Some(Wanted {
                plugin: plugin.to_owned(),
                module: module.to_owned(),
                config: config.to_vec(),
            })
        });
    want(wanted);
}

fn want(wanted: Option<Wanted>) {
    let step = STATE.with_borrow_mut(|state| state.loader.want(wanted.clone()));
    match step {
        Step::Nothing => {}
        Step::Stop => {
            with_filter(|filter| {
                let outputs = filter
                    .remapper
                    .as_mut()
                    .map(|remapper| remapper.configure(None))
                    .unwrap_or_default();
                filter.remapper = None;
                outputs
            });
            STATE.with_borrow_mut(|state| state.save = Some(None));
        }
        Step::Configure(config) => {
            with_remapper(|remapper, _| remapper.configure(Some(config)));
            remember();
        }
        Step::Fetch { plugin, module } => fetch(plugin, module),
    }
}

fn fetch(plugin: String, module: String) {
    if !offers_remapper(&plugin) {
        block_app_base::notices::report(format!(
            "The input remaps name {plugin}, which is not a plugin that remaps input here."
        ));
        return;
    }
    let fetched = STATE.with_borrow(|state| Arc::clone(&state.fetched));
    let path = module.clone();
    discovery::data(&plugin.clone(), &path, move |read| {
        if let Ok(mut slot) = fetched.lock() {
            *slot = Some((plugin, module, read));
        }
        block_app_base::host::wake();
    });
}

fn offers_remapper(plugin: &str) -> bool {
    let remapper = *InputRemapper::CONTENT_TYPE.as_bytes();
    discovery::manifests().iter().any(|manifest| {
        manifest.identity.id == plugin
            && manifest
                .editors
                .iter()
                .any(|editor| editor.edits == block_plugin_api::Edits::Type(remapper))
    })
}

fn take_fetched() {
    let fetched =
        STATE.with_borrow(|state| state.fetched.lock().ok().and_then(|mut slot| slot.take()));
    let Some((plugin, module, read)) = fetched else {
        return;
    };
    let bytes = match read {
        Ok(bytes) => bytes,
        Err(error) => {
            block_app_base::notices::report(format!("The input remapper could not be read: {error}"));
            return;
        }
    };
    let Some(wanted) = STATE.with_borrow_mut(|state| state.loader.fetched(&plugin, &module)) else {
        return;
    };
    if install(&wanted, &bytes) {
        remember();
    }
}

fn remember() {
    STATE.with_borrow_mut(|state| {
        let Some(wanted) = state.loader.wanted().cloned() else {
            return;
        };
        state.save = Some(Some(encode(&(
            wanted.plugin,
            wanted.module,
            state.module.clone(),
            wanted.config,
        ))));
    });
}

fn install(wanted: &Wanted, module: &[u8]) -> bool {
    let mut remapper = match Remapper::new(module) {
        Ok(remapper) => remapper,
        Err(error) => {
            block_app_base::notices::report(format!("The input remapper could not be loaded: {error}"));
            return false;
        }
    };
    let (window, locked) = STATE.with_borrow_mut(|state| {
        state.module = module.to_vec();
        (state.window.clone(), state.locked)
    });
    remapper.configure(Some(wanted.config.clone()));
    remapper.set_locked(locked);
    with_filter(|filter| {
        let now = micros(filter.now);
        let mut outputs = filter
            .remapper
            .as_mut()
            .map(|old| old.configure(None))
            .unwrap_or_default();
        outputs.extend(remapper.focus(window, now));
        filter.remapper = Some(remapper);
        outputs
    });
    true
}

fn with_remapper(change: impl FnOnce(&mut Remapper, u64) -> Vec<Output>) {
    with_filter(|filter| {
        let now = micros(filter.now);
        let outputs = filter
            .remapper
            .as_mut()
            .map(|remapper| change(remapper, now))
            .unwrap_or_default();
        filter.report();
        outputs
    });
}

fn with_filter(change: impl FnOnce(&mut Filter) -> Vec<Output>) {
    let (control, filter) =
        STATE.with_borrow(|state| (state.control.clone(), state.filter.clone()));
    let (Some(control), Some(filter)) = (control, filter) else {
        return;
    };
    let outputs = change(&mut filter.borrow_mut());
    control.inject(outputs.into_iter().map(raw).collect());
}

fn micros(now: Duration) -> u64 {
    u64::try_from(now.as_micros()).unwrap_or(u64::MAX)
}

fn remap_input(input: RawInput) -> Option<Input> {
    let source = |code: u32| u16::try_from(code).ok().map(Source::from_code);
    Some(match input {
        RawInput::Key { code, pressed } if code < BUTTON_FIRST => match pressed {
            true => Input::Press(source(code)?),
            false => Input::Release(source(code)?),
        },
        RawInput::Button { code, pressed } if code >= BUTTON_FIRST => match pressed {
            true => Input::Press(source(code)?),
            false => Input::Release(source(code)?),
        },
        RawInput::Wheel { x, y } => Input::Wheel { x, y },
        RawInput::Key { .. } | RawInput::Button { .. } => return None,
    })
}

fn raw(output: Output) -> RawInput {
    let pressed = matches!(output, Input::Press(_));
    match output {
        Input::Press(Source::Key(code)) | Input::Release(Source::Key(code)) => RawInput::Key {
            code: code.into(),
            pressed,
        },
        Input::Press(Source::Button(code)) | Input::Release(Source::Button(code)) => {
            RawInput::Button {
                code: code.into(),
                pressed,
            }
        }
        Input::Wheel { x, y } => RawInput::Wheel { x, y },
    }
}
