use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use be_block::InputSettings;
use be_block::input_settings::RemapGroup;
use be_remap_host::Remapper;
use be_remap_host::be_remap::rules::exe_name;
use be_remap_host::be_remap::{Input, Output, Source, Window, protocol};
use beui_adapter_drm::{InputControl, InputFilter, RawInput};
use block_plugin_api::{RemapRequest, RemapState, Remaps};

use crate::ui::RemapsPrompt;
use approval::Approval;

mod approval;

const MODULE: &[u8] = include_bytes!(env!("BE_REMAP_WASM"));
const BUTTON_FIRST: u32 = 0x100;

#[derive(Default)]
struct State {
    control: Option<InputControl>,
    filter: Option<Rc<RefCell<Filter>>>,
    approval: Approval,
    text: String,
    groups: usize,
    published: Option<RemapState>,
}

struct Filter {
    remapper: Remapper,
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
        let outputs = self.remapper.input(remapped, micros(now));
        self.report();
        outputs.into_iter().map(raw).collect()
    }

    fn timer(&mut self, now: Duration) -> Vec<RawInput> {
        self.now = now;
        let outputs = self.remapper.timer(micros(now));
        self.report();
        outputs.into_iter().map(raw).collect()
    }

    fn deadline(&self) -> Option<Duration> {
        self.remapper.deadline().map(Duration::from_micros)
    }

    fn forget_held(&mut self) {
        self.remapper.forget_held();
    }
}

impl Filter {
    fn report(&mut self) {
        for problem in self.remapper.take_problems() {
            crate::notices::report(problem);
        }
    }
}

pub(crate) fn start(control: Option<InputControl>) {
    let Some(control) = control else {
        return;
    };
    let remapper = match Remapper::new(MODULE) {
        Ok(remapper) => remapper,
        Err(error) => {
            crate::notices::report(format!("The input remaps could not be loaded: {error}"));
            return;
        }
    };
    let filter = Rc::new(RefCell::new(Filter {
        remapper,
        now: Duration::ZERO,
    }));
    control.set_filter(Some(filter.clone()));
    STATE.with_borrow_mut(|state| {
        state.control = Some(control);
        state.filter = Some(filter);
    });
    refresh();
}

pub(crate) fn load_approved(approved: Option<Vec<u8>>) {
    STATE.with_borrow_mut(|state| state.approval.load(approved));
    refresh();
}

pub(crate) fn configure(settings: &InputSettings) {
    let groups: Vec<RemapGroup> = match settings.remap_groups() {
        Ok(groups) => groups,
        Err(problem) => {
            crate::notices::report(format!(
                "The remaps in the input settings could not be read, so none run: {problem}"
            ));
            Vec::new()
        }
    }
    .into_iter()
    .filter(|group| !group.rules.is_empty())
    .collect();
    let wanted = (!groups.is_empty()).then(|| protocol::config(&groups));
    let changed = STATE.with_borrow_mut(|state| {
        let changed = state.approval.want(wanted);
        state.text = settings.remaps.clone().unwrap_or_default();
        state.groups = groups.len();
        changed
    });
    if changed {
        refresh();
    }
}

pub(crate) fn focus(window: Option<be_wayland::WindowInfo>) {
    let window = window.map(|window| Window {
        exe: exe_name(&window.command),
        app_id: window.app_id,
        title: window.title,
    });
    with_filter(|filter| {
        let now = micros(filter.now);
        let outputs = filter.remapper.focus(window, now);
        filter.report();
        outputs
    });
}

pub(crate) fn set_locked(locked: bool) {
    with_filter(|filter| filter.remapper.set_locked(locked));
}

pub(crate) fn frame() {
    for request in crate::plugin_host::take_actions::<RemapRequest>() {
        match request {
            RemapRequest::Review => STATE.with_borrow_mut(|state| state.approval.review()),
        }
    }
    let shown = STATE.with_borrow(|state| {
        let stopped = state
            .filter
            .as_ref()
            .is_some_and(|filter| filter.borrow().remapper.stopped());
        let value = match state.filter.is_some() {
            true => state.approval.state(stopped),
            false => RemapState::Off,
        };
        (state.published != Some(value)).then_some(value)
    });
    if let Some(value) = shown {
        crate::plugin_host::publish::<Remaps>(&value);
        STATE.with_borrow_mut(|state| state.published = Some(value));
    }
}

pub(crate) fn prompt() -> Option<RemapsPrompt> {
    STATE.with_borrow(|state| {
        (state.filter.is_some() && state.approval.asking()).then(|| RemapsPrompt {
            text: state.text.clone(),
            groups: state.groups,
        })
    })
}

pub(crate) fn turn_on() -> Option<Vec<u8>> {
    let approved = STATE.with_borrow_mut(|state| state.approval.turn_on());
    refresh();
    approved
}

pub(crate) fn keep_off() {
    STATE.with_borrow_mut(|state| state.approval.keep_off());
}

fn refresh() {
    let running = STATE.with_borrow(|state| state.approval.running());
    with_filter(|filter| filter.remapper.configure(running));
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
