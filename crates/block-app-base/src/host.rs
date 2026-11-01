use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::{Mutex, OnceLock, PoisonError},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

use ui_types::{FileFilter, FilePick, FileSave, SavedFile};

type Waker = Box<dyn Fn() + Send + Sync>;

static WAKER: OnceLock<Waker> = OnceLock::new();
static UNSENT_SAVES: Mutex<Vec<(SavedFile, SaveSlot)>> = Mutex::new(Vec::new());

type Letter = (u64, Box<dyn Any + Send>);
type Receive = Rc<dyn Fn(&mut dyn Any, Box<dyn Any + Send>)>;

static POSTED: Mutex<Vec<(ThreadId, Letter)>> = Mutex::new(Vec::new());

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
    static MAILBOXES: RefCell<HashMap<u64, (u64, Receive)>> = RefCell::new(HashMap::new());
    static NEXT_MAILBOX: Cell<u64> = const { Cell::new(1) };
}

pub fn install_waker(waker: impl Fn() + Send + Sync + 'static) {
    let _ = WAKER.set(Box::new(waker));
}

static CLOCK: OnceLock<fn() -> Instant> = OnceLock::new();

pub fn install_clock(clock: fn() -> Instant) {
    let _ = CLOCK.set(clock);
}

pub fn clock() -> Instant {
    CLOCK.get().map_or_else(Instant::now, |clock| clock())
}

pub fn wake() {
    if let Some(waker) = WAKER.get() {
        waker();
    }
}

pub struct Mailbox(u64);

impl Drop for Mailbox {
    fn drop(&mut self) {
        let _ = MAILBOXES.try_with(|mailboxes| mailboxes.borrow_mut().remove(&self.0));
    }
}

pub struct Post<T> {
    mailbox: u64,
    thread: ThreadId,
    kind: PhantomData<fn(T)>,
}

impl<T> Clone for Post<T> {
    fn clone(&self) -> Self {
        Self {
            mailbox: self.mailbox,
            thread: self.thread,
            kind: PhantomData,
        }
    }
}

impl<T: Send + 'static> Post<T> {
    pub fn send(&self, value: T) {
        POSTED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((self.thread, (self.mailbox, Box::new(value))));
        wake();
    }
}

fn next_mailbox() -> u64 {
    let next = NEXT_MAILBOX.get();
    NEXT_MAILBOX.set(next + 1);
    next
}

pub struct Desk<C> {
    id: u64,
    context: PhantomData<fn(&mut C)>,
}

impl<C> Clone for Desk<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Desk<C> {}

impl<C: 'static> Default for Desk<C> {
    fn default() -> Self {
        Self {
            id: next_mailbox(),
            context: PhantomData,
        }
    }
}

impl<C: 'static> Desk<C> {
    pub fn mailbox<T: Send + 'static>(
        self,
        receive: impl Fn(&mut C, T) + 'static,
    ) -> (Mailbox, Post<T>) {
        let mailbox = next_mailbox();
        let receive: Receive = Rc::new(move |context, letter| {
            if let (Some(context), Ok(value)) =
                (context.downcast_mut::<C>(), letter.downcast::<T>())
            {
                receive(context, *value);
            }
        });
        MAILBOXES.with_borrow_mut(|mailboxes| mailboxes.insert(mailbox, (self.id, receive)));
        let post = Post {
            mailbox,
            thread: thread::current().id(),
            kind: PhantomData,
        };
        (Mailbox(mailbox), post)
    }

    pub fn deliver(self, context: &mut C) {
        let here = thread::current().id();
        let at_this_desk = |mailbox: u64| {
            MAILBOXES
                .with_borrow(|mailboxes| mailboxes.get(&mailbox).map(|(desk, _)| *desk == self.id))
        };
        let letters: Vec<Letter> = {
            let mut posted = POSTED.lock().unwrap_or_else(PoisonError::into_inner);
            let mut letters = Vec::new();
            posted.retain_mut(|(thread, letter)| {
                if *thread != here {
                    return true;
                }
                match at_this_desk(letter.0) {
                    Some(true) => {
                        letters.push(std::mem::replace(letter, (0, Box::new(()))));
                        false
                    }
                    Some(false) => true,
                    None => false,
                }
            });
            letters
        };
        for (mailbox, letter) in letters {
            let receive = MAILBOXES.with_borrow(|mailboxes| {
                mailboxes
                    .get(&mailbox)
                    .map(|(_, receive)| Rc::clone(receive))
            });
            if let Some(receive) = receive {
                receive(context, letter);
            }
        }
    }
}

const FRAME: Desk<()> = Desk {
    id: 0,
    context: PhantomData,
};

pub fn mailbox<T: Send + 'static>(receive: impl Fn(T) + 'static) -> (Mailbox, Post<T>) {
    FRAME.mailbox(move |(), value| receive(value))
}

pub fn deliver() {
    FRAME.deliver(&mut ());
}

#[derive(Clone, Debug)]
pub enum HostCommand {
    RestartPlugin(String),
}

#[derive(Default)]
pub struct Output {
    pub fullscreen: Option<bool>,
    pub copied: Option<String>,
    pub copied_image: Option<Vec<u8>>,
    pub file_picks: Vec<(FileFilter, PickSlot)>,
    pub paste: bool,
    pub grab: Option<bool>,
    pub repaint: bool,
    pub repaint_after: Option<Duration>,
    pub animating: bool,
    pub close: bool,
}

struct Host {
    started: Option<Instant>,
    now: Duration,
    wall: Duration,
    pass: u64,
    seen: bool,
    clock_stopped: bool,
    pixels_per_point: f32,
    screen_scale: f32,
    opens_windows: bool,
    preferences: block_plugin_api::Preferences,
    commands: Vec<HostCommand>,
    changed: bool,
    output: Output,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            started: None,
            now: Duration::ZERO,
            wall: Duration::ZERO,
            pass: 0,
            seen: true,
            clock_stopped: false,
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            opens_windows: false,
            preferences: block_plugin_api::Preferences::default(),
            commands: Vec::new(),
            changed: false,
            output: Output::default(),
        }
    }
}

fn with<R>(act: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|host| act(&mut host.borrow_mut()))
}

pub struct Frame {
    pub now: Option<Instant>,
    pub wall: Option<Duration>,
    pub seen: bool,
    pub clock_stopped: bool,
    pub preferences: block_plugin_api::Preferences,
    pub pixels_per_point: f32,
    pub screen_scale: f32,
    pub opens_windows: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            now: None,
            wall: None,
            seen: true,
            clock_stopped: false,
            preferences: block_plugin_api::Preferences::default(),
            pixels_per_point: 1.0,
            screen_scale: 1.0,
            opens_windows: false,
        }
    }
}

pub fn start(frame: Frame) {
    with(|host| {
        host.preferences = frame.preferences;
        host.pass += 1;
        host.seen = frame.seen;
        host.clock_stopped = frame.clock_stopped;
        if let Some(now) = frame.now {
            let started = *host.started.get_or_insert(now);
            host.now = now.saturating_duration_since(started);
        }
        if let Some(wall) = frame.wall {
            host.wall = wall;
        }
        host.pixels_per_point = frame.pixels_per_point;
        host.screen_scale = frame.screen_scale;
        host.opens_windows = frame.opens_windows;
        host.output = Output::default();
    });
    deliver();
}

pub fn finish() -> Output {
    with(|host| std::mem::take(&mut host.output))
}

pub fn take_unsent_saves() -> Vec<(SavedFile, SaveSlot)> {
    std::mem::take(&mut *UNSENT_SAVES.lock().unwrap_or_else(PoisonError::into_inner))
}

pub fn changed() {
    with(|host| host.changed = true);
    request_repaint();
}

pub fn take_changed() -> bool {
    with(|host| std::mem::take(&mut host.changed))
}

pub fn pass() -> u64 {
    with(|host| host.pass)
}

pub fn seen() -> bool {
    with(|host| host.seen)
}

pub fn clock_stopped() -> bool {
    with(|host| host.clock_stopped)
}

pub fn now() -> Duration {
    with(|host| host.now)
}

pub fn wall_clock() -> Duration {
    with(|host| host.wall)
}

pub fn micros_at(at: Instant) -> u64 {
    with(|host| match host.started {
        Some(started) => at.saturating_duration_since(started),
        None => host.now,
    })
    .as_micros() as u64
}

pub fn milliseconds() -> u64 {
    now().as_millis() as u64
}

pub fn pixels_per_point() -> f32 {
    with(|host| host.pixels_per_point)
}

pub fn opens_windows() -> bool {
    with(|host| host.opens_windows)
}

pub fn preferences() -> block_plugin_api::Preferences {
    with(|host| host.preferences)
}

pub fn set_fullscreen(fullscreen: bool) {
    with(|host| host.output.fullscreen = Some(fullscreen));
}

pub fn set_grab(grab: bool) {
    with(|host| host.output.grab = Some(grab));
}

pub fn copy_text(text: String) {
    with(|host| {
        host.output.copied_image = None;
        host.output.copied = Some(text);
    });
}

pub fn copy_image(png: Vec<u8>) {
    with(|host| {
        host.output.copied = None;
        host.output.copied_image = Some(png);
    });
}

pub type PickSlot = Option<Box<dyn FnOnce(FilePick)>>;
pub type SaveSlot = Box<dyn FnOnce(FileSave) + Send>;

pub fn request_paste() {
    with(|host| host.output.paste = true);
}

pub fn pick_file(filter: FileFilter, deliver: impl FnOnce(FilePick) + 'static) {
    with(|host| {
        host.output
            .file_picks
            .push((filter, Some(Box::new(deliver))))
    });
}

pub fn save_file(file: SavedFile, deliver: impl FnOnce(FileSave) + Send + 'static) {
    UNSENT_SAVES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((file, Box::new(deliver)));
    wake();
}

pub fn request_repaint() {
    with(|host| host.output.repaint = true);
}

pub fn request_repaint_after(delay: Duration) {
    with(|host| {
        host.output.repaint_after = Some(match host.output.repaint_after {
            Some(current) => current.min(delay),
            None => delay,
        });
    });
}

pub fn close_window() {
    with(|host| host.output.close = true);
}

pub fn note_animating() {
    with(|host| host.output.animating = true);
}

pub fn push_command(command: HostCommand) {
    with(|host| host.commands.push(command));
    wake();
}

pub fn take_commands() -> Vec<HostCommand> {
    with(|host| std::mem::take(&mut host.commands))
}

#[cfg(test)]
mod tests;
