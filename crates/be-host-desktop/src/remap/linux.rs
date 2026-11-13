use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use be_block::be_block_model::Document;
use be_block::{BlockContent, InputRemapper, InputSettings, Root};
use be_host_remap_host::Remapper;
use be_host_remap_host::be_lib_remap::Wheel;
use be_host_remap_host::be_lib_remap::protocol::{decode, encode};
use be_host_remap_host::be_lib_remap::rules::exe_name;
use be_host_remap_host::be_lib_remap::{Input, Output, Source, Window};
use be_host_remap_host::capture::{Capture, Caught};
use be_sdk_api::{
    CapturedChord, CapturedTrigger, InputCapture, InputCaptureWatch, RemapState, RemapsWatch,
    WheelTurn,
};
use beui_platform_drm::{InputControl, InputFilter, RawInput};
use uuid::Uuid;

use be_host_base::host_state::HostStateStore;
use be_host_stack::discovery;
use loader::{Loader, Step, Wanted};

mod loader;

const BUTTON_FIRST: u32 = 0x100;

type Fetched = Arc<Mutex<Option<(String, String, Result<Vec<u8>, String>)>>>;

#[derive(Default)]
struct State {
    control: Option<InputControl>,
    filter: Option<Rc<RefCell<Filter>>>,
    chosen: Option<Uuid>,
    /// Whether the chosen block changed since it was read.
    due: bool,
    /// What the host stack changed, which says when the chosen block did.
    changes: be_host_stack::ChangeWatch,
    loader: Loader,
    fetched: Fetched,
    window: Option<Window>,
    locked: bool,
    save: Option<Option<Vec<u8>>>,
    module: Vec<u8>,
    published: Option<RemapState>,
    capture_openings: u64,
}

#[derive(Default)]
struct Filter {
    capture: Capture,
    remapper: Option<Remapper>,
    now: Duration,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

impl InputFilter for Filter {
    fn input(&mut self, input: RawInput, now: Duration) -> Vec<RawInput> {
        self.now = now;
        let Some(remapped) = remap_input(input) else {
            return vec![input];
        };
        if self.capture.catching() {
            be_host_base::host::wake();
        }
        let passed = self.capture.input(remapped);
        let Some(remapper) = self.remapper.as_mut() else {
            return passed.into_iter().map(raw).collect();
        };
        let outputs: Vec<Output> = passed
            .into_iter()
            .flat_map(|input| remapper.input(input, micros(now)))
            .collect();
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
        self.capture.forget_held();
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
            be_host_base::notices::report(problem);
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
        .map(|(app, module_path, module, config)| {
            (
                Wanted {
                    app,
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
        state.due = true;
        chosen.is_none()
    });
    if cleared {
        want(None);
    }
}

pub fn focus(window: Option<be_host_wayland::WindowInfo>) {
    let window = window.map(|window| Window {
        exe: exe_name(&window.command),
        program_id: window.program_id,
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
    STATE.with_borrow_mut(|state| {
        state.locked = locked;
        if let Some(filter) = &state.filter {
            filter.borrow_mut().capture.set_locked(locked);
        }
    });
    with_remapper(|remapper, _| remapper.set_locked(locked));
}

pub fn frame(store: &HostStateStore) {
    read_chosen();
    take_fetched();
    if let Some(save) = STATE.with_borrow_mut(|state| state.save.take())
        && let Err(error) = store.set_remapper(save.as_deref())
    {
        eprintln!("be-host: the input remapper was not kept for the next session: {error}");
    }
    let shown = STATE.with_borrow(|state| {
        let filter = state.filter.as_ref()?.borrow();
        let value = match filter.remapper.as_ref().map(Remapper::stopped) {
            Some(false) => RemapState::On,
            Some(true) => RemapState::Stopped,
            None => RemapState::Off,
        };
        (state.published != Some(value)).then_some(value)
    });
    if let Some(value) = shown {
        be_host_base::watches::publish::<RemapsWatch>(&value);
        STATE.with_borrow_mut(|state| state.published = Some(value));
    }
    capture();
}

/// Starts a capture at each opening of an `InputCaptureWatch`, stops it once none is open,
/// and tells the watch what it caught.
fn capture() {
    let watched = be_host_base::watches::watched::<InputCaptureWatch>();
    let openings = be_host_base::watches::openings::<InputCaptureWatch>();
    let value = STATE.with_borrow_mut(|state| {
        let filter = state.filter.as_ref()?;
        let mut filter = filter.borrow_mut();
        let capture = &mut filter.capture;
        let opened = std::mem::replace(&mut state.capture_openings, openings) != openings;
        if watched && opened {
            capture.start();
        } else if !watched && capture.catching() {
            capture.stop();
        }
        watched.then(|| InputCapture {
            held: capture.held().iter().map(|source| source.code()).collect(),
            chord: capture.chord().map(|chord| CapturedChord {
                held: chord.held.iter().map(|source| source.code()).collect(),
                trigger: match chord.trigger {
                    Caught::Source(source) => CapturedTrigger::Source(source.code()),
                    Caught::Wheel(wheel) => CapturedTrigger::Wheel(match wheel {
                        Wheel::Up => WheelTurn::Up,
                        Wheel::Down => WheelTurn::Down,
                        Wheel::Left => WheelTurn::Left,
                        Wheel::Right => WheelTurn::Right,
                    }),
                },
            }),
        })
    });
    if let Some(value) = value {
        be_host_base::watches::publish::<InputCaptureWatch>(&value);
    }
}

fn read_chosen() {
    let changes = STATE.with_borrow(|state| state.changes.take());
    let Some(block) = STATE.with_borrow(|state| state.chosen) else {
        return;
    };
    let due = STATE.with_borrow_mut(|state| std::mem::take(&mut state.due) || changes.block(block));
    if !due {
        return;
    }
    be_host_stack::hold(block, InputRemapper::CONTENT_TYPE);
    // One not loaded yet is read when it arrives, which the stack tells of.
    let Some(content) = be_host_stack::content(block) else {
        return;
    };
    let wanted = Document::<InputRemapper>::decode(content.bytes())
        .ok()
        .and_then(|content| {
            let remapper = content.root();
            let (app, module, config) = remapper.runs()?;
            Some(Wanted {
                app: app.to_owned(),
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
        Step::Fetch { app, module } => fetch(app, module),
    }
}

fn fetch(app: String, module: String) {
    if !offers_remapper(&app) {
        be_host_base::notices::report(format!(
            "The input remaps name {app}, which is not an app that remaps input here."
        ));
        STATE.with_borrow_mut(|state| state.loader.failed());
        return;
    }
    let fetched = STATE.with_borrow(|state| Arc::clone(&state.fetched));
    let path = module.clone();
    discovery::data(&app.clone(), &path, move |read| {
        if let Ok(mut slot) = fetched.lock() {
            *slot = Some((app, module, read));
        }
        be_host_base::host::wake();
    });
}

fn offers_remapper(app: &str) -> bool {
    let remapper = *InputRemapper::CONTENT_TYPE.as_bytes();
    discovery::manifests().iter().any(|manifest| {
        manifest.identity.id == app
            && manifest
                .editors
                .iter()
                .any(|editor| editor.edits == be_sdk_api::Edits::Type(remapper))
    })
}

fn take_fetched() {
    let fetched =
        STATE.with_borrow(|state| state.fetched.lock().ok().and_then(|mut slot| slot.take()));
    let Some((app, module, read)) = fetched else {
        return;
    };
    let bytes = match read {
        Ok(bytes) => bytes,
        Err(error) => {
            be_host_base::notices::report(format!("The input remapper could not be read: {error}"));
            STATE.with_borrow_mut(|state| {
                if state.loader.fetching(&app, &module) {
                    state.loader.failed();
                }
            });
            return;
        }
    };
    let Some(wanted) = STATE.with_borrow_mut(|state| state.loader.fetched(&app, &module)) else {
        return;
    };
    match install(&wanted, &bytes) {
        true => remember(),
        false => STATE.with_borrow_mut(|state| state.loader.failed()),
    }
}

fn remember() {
    STATE.with_borrow_mut(|state| {
        let Some(wanted) = state.loader.wanted().cloned() else {
            return;
        };
        state.save = Some(Some(encode(&(
            wanted.app,
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
            be_host_base::notices::report(format!(
                "The input remapper could not be loaded: {error}"
            ));
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
