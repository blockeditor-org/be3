//! The WASI preview1 calls an app's standard library and C make, as crates/be-host/web/wasi.js
//! answers them in the browser: no files, arguments or environment; standard error to the host's,
//! standard output dropped and standard input empty; the clocks, randomness, yielding and sleeping
//! on a clock; and exiting, which stops the app. Any other call is unsupported (ENOSYS). A pure
//! guest gets none of them.

use std::{
    io::Write,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime},
};

use crate::engine::Defined;
use crate::gpu::throw;
use crate::state::{View, state_of};

pub(crate) const MODULE: &str = "wasi_snapshot_preview1";

const SUCCESS: i32 = 0;
const EBADF: i32 = 8;
const EINVAL: i32 = 28;
const ENOENT: i32 = 44;
const ENOSYS: i32 = 52;
const ESPIPE: i32 = 70;

const STDIN: i32 = 0;
const STDOUT: i32 = 1;
const STDERR: i32 = 2;
const FILETYPE_CHARACTER_DEVICE: u8 = 2;
const REALTIME: i32 = 0;
const SUBSCRIPTION: u32 = 48;
const EVENT: u32 = 32;
const CLOCK: u8 = 0;
const ABSOLUTE: u16 = 1;

type Call = fn(&mut v8::PinScope, &Arguments, &View) -> Result<i32, String>;

/// One call's arguments.
struct Arguments<'a, 's>(&'a v8::FunctionCallbackArguments<'s>);

impl Arguments<'_, '_> {
    fn u32(&self, scope: &mut v8::PinScope, index: i32) -> u32 {
        self.0.get(index).uint32_value(scope).unwrap_or(0)
    }

    fn i32(&self, scope: &mut v8::PinScope, index: i32) -> i32 {
        self.0.get(index).int32_value(scope).unwrap_or(0)
    }
}

fn write_u32(view: &View, at: u32, value: u32) -> Result<(), String> {
    view.write(at, &value.to_le_bytes())
}

fn write_u64(view: &View, at: u32, value: u64) -> Result<(), String> {
    view.write(at, &value.to_le_bytes())
}

fn read_u32(view: &View, at: u32) -> Result<u32, String> {
    let bytes = view.read(at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64(view: &View, at: u32) -> Result<u64, String> {
    let bytes = view.read(at, 8)?;
    let mut word = [0; 8];
    word.copy_from_slice(&bytes);
    Ok(u64::from_le_bytes(word))
}

fn stream(fd: i32) -> bool {
    matches!(fd, STDIN | STDOUT | STDERR)
}

fn fd_write(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    let fd = arguments.i32(scope, 0);
    let (iovs, count, written) = (
        arguments.u32(scope, 1),
        arguments.u32(scope, 2),
        arguments.u32(scope, 3),
    );
    if fd != STDOUT && fd != STDERR {
        return Ok(EBADF);
    }
    let mut total = 0u32;
    let mut stderr = std::io::stderr().lock();
    for index in 0..count {
        let entry = iovs.wrapping_add(index.wrapping_mul(8));
        let (pointer, length) = (read_u32(view, entry)?, read_u32(view, entry + 4)?);
        let bytes = view.read(pointer, length)?;
        if fd == STDERR {
            let _ = stderr.write_all(&bytes);
        }
        total = total.wrapping_add(length);
    }
    let _ = stderr.flush();
    write_u32(view, written, total)?;
    Ok(SUCCESS)
}

fn fd_read(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    if arguments.i32(scope, 0) != STDIN {
        return Ok(EBADF);
    }
    write_u32(view, arguments.u32(scope, 3), 0)?;
    Ok(SUCCESS)
}

fn on_streams(scope: &mut v8::PinScope, arguments: &Arguments, _: &View) -> Result<i32, String> {
    Ok(match stream(arguments.i32(scope, 0)) {
        true => SUCCESS,
        false => EBADF,
    })
}

fn fd_seek(scope: &mut v8::PinScope, arguments: &Arguments, _: &View) -> Result<i32, String> {
    Ok(match stream(arguments.i32(scope, 0)) {
        true => ESPIPE,
        false => EBADF,
    })
}

fn fd_fdstat_get(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    if !stream(arguments.i32(scope, 0)) {
        return Ok(EBADF);
    }
    let at = arguments.u32(scope, 1);
    let mut fdstat = [0u8; 24];
    fdstat[0] = FILETYPE_CHARACTER_DEVICE;
    view.write(at, &fdstat)?;
    Ok(SUCCESS)
}

fn bad_descriptor(_: &mut v8::PinScope, _: &Arguments, _: &View) -> Result<i32, String> {
    Ok(EBADF)
}

fn no_entry(_: &mut v8::PinScope, _: &Arguments, _: &View) -> Result<i32, String> {
    Ok(ENOENT)
}

fn none_counted(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    write_u32(view, arguments.u32(scope, 0), 0)?;
    write_u32(view, arguments.u32(scope, 1), 0)?;
    Ok(SUCCESS)
}

fn nothing(_: &mut v8::PinScope, _: &Arguments, _: &View) -> Result<i32, String> {
    Ok(SUCCESS)
}

/// The monotonic clock in nanoseconds, from when the process first asked.
fn monotonic() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as u64
}

fn now(clock: i32) -> u64 {
    match clock {
        REALTIME => SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos() as u64),
        _ => monotonic(),
    }
}

fn clock_time_get(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    let clock = arguments.i32(scope, 0);
    if !(0..=3).contains(&clock) {
        return Ok(EINVAL);
    }
    write_u64(view, arguments.u32(scope, 2), now(clock))?;
    Ok(SUCCESS)
}

fn clock_res_get(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    write_u64(view, arguments.u32(scope, 1), 1)?;
    Ok(SUCCESS)
}

fn random_get(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    let (pointer, length) = (arguments.u32(scope, 0), arguments.u32(scope, 1));
    let mut bytes = vec![0u8; length as usize];
    getrandom::fill(&mut bytes).map_err(|error| format!("no randomness for the app: {error}"))?;
    view.write(pointer, &bytes)?;
    Ok(SUCCESS)
}

fn sched_yield(_: &mut v8::PinScope, _: &Arguments, _: &View) -> Result<i32, String> {
    std::thread::yield_now();
    Ok(SUCCESS)
}

/// Waits on clocks only: sleeps until the soonest subscription is due, then reports every one as
/// having fired.
fn poll_oneoff(scope: &mut v8::PinScope, arguments: &Arguments, view: &View) -> Result<i32, String> {
    let (subscriptions, events, count, written) = (
        arguments.u32(scope, 0),
        arguments.u32(scope, 1),
        arguments.u32(scope, 2),
        arguments.u32(scope, 3),
    );
    let mut soonest: Option<u64> = None;
    for index in 0..count {
        let at = subscriptions.wrapping_add(index.wrapping_mul(SUBSCRIPTION));
        if view.read(at + 8, 1)?[0] != CLOCK {
            continue;
        }
        let clock = read_u32(view, at + 16)? as i32;
        let timeout = read_u64(view, at + 24)?;
        let flags = u16::from_le_bytes(view.read(at + 40, 2)?.try_into().unwrap_or([0; 2]));
        let wait = match flags & ABSOLUTE {
            0 => timeout,
            _ => timeout.saturating_sub(now(clock)),
        };
        soonest = Some(soonest.map_or(wait, |soonest| soonest.min(wait)));
    }
    if let Some(wait) = soonest {
        std::thread::sleep(Duration::from_nanos(wait));
    }
    for index in 0..count {
        let from = subscriptions.wrapping_add(index.wrapping_mul(SUBSCRIPTION));
        let at = events.wrapping_add(index.wrapping_mul(EVENT));
        let mut event = [0u8; EVENT as usize];
        event[..8].copy_from_slice(&read_u64(view, from)?.to_le_bytes());
        event[10] = view.read(from + 8, 1)?[0];
        view.write(at, &event)?;
    }
    write_u32(view, written, count)?;
    Ok(SUCCESS)
}

fn proc_exit(scope: &mut v8::PinScope, arguments: &Arguments, _: &View) -> Result<i32, String> {
    Err(format!("the app exited with status {}", arguments.i32(scope, 0)))
}

fn answer(
    scope: &mut v8::PinScope,
    arguments: &v8::FunctionCallbackArguments,
    returned: &mut v8::ReturnValue<v8::Value>,
    call: Call,
) {
    // unsafe(ffi): the function's data is its State (state_of).
    let state = unsafe { state_of(arguments.data()) };
    let view = state.view_in(scope);
    match call(scope, &Arguments(arguments), &view) {
        Ok(errno) => returned.set_int32(errno),
        Err(stopped) => {
            state.fatal = Some(stopped.clone());
            throw(scope, &stopped);
        }
    }
}

fn unsupported_call(
    _: &mut v8::PinScope,
    _: v8::FunctionCallbackArguments,
    mut returned: v8::ReturnValue<v8::Value>,
) {
    returned.set_int32(ENOSYS);
}

macro_rules! define_calls {
    ($scope:ident, $defined:ident, $data:ident, $name:expr, $($call:ident)*) => {
        match $name {
            $(stringify!($call) => {
                fn call(
                    scope: &mut v8::PinScope,
                    arguments: v8::FunctionCallbackArguments,
                    mut returned: v8::ReturnValue<v8::Value>,
                ) {
                    answer(scope, &arguments, &mut returned, $call);
                }
                crate::engine::function($scope, $defined, MODULE, $name, $data, call)
            })*
            "args_get" | "environ_get" => {
                crate::engine::function($scope, $defined, MODULE, $name, $data, succeed)
            }
            "args_sizes_get" | "environ_sizes_get" => {
                crate::engine::function($scope, $defined, MODULE, $name, $data, none_counted_call)
            }
            "fd_close" | "fd_fdstat_set_flags" | "fd_sync" => {
                crate::engine::function($scope, $defined, MODULE, $name, $data, on_streams_call)
            }
            "fd_filestat_get" | "fd_prestat_dir_name" | "fd_prestat_get" | "fd_readdir" => {
                crate::engine::function($scope, $defined, MODULE, $name, $data, bad_descriptor_call)
            }
            "path_filestat_get" | "path_open" | "path_remove_directory" | "path_unlink_file" => {
                crate::engine::function($scope, $defined, MODULE, $name, $data, no_entry_call)
            }
            _ => crate::engine::function($scope, $defined, MODULE, $name, $data, unsupported_call),
        }
    };
}

macro_rules! answered {
    ($($wrapper:ident => $call:ident),*) => {
        $(fn $wrapper(
            scope: &mut v8::PinScope,
            arguments: v8::FunctionCallbackArguments,
            mut returned: v8::ReturnValue<v8::Value>,
        ) {
            answer(scope, &arguments, &mut returned, $call);
        })*
    };
}

answered!(
    succeed => nothing,
    none_counted_call => none_counted,
    on_streams_call => on_streams,
    bad_descriptor_call => bad_descriptor,
    no_entry_call => no_entry
);

/// Every WASI call `imports` names.
pub(crate) fn define<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    data: v8::Local<'s, v8::Value>,
    imports: &[crate::engine::Import],
) -> Result<(), String> {
    for import in imports.iter().filter(|import| import.module == MODULE) {
        let name = import.name.as_str();
        define_calls!(
            scope, defined, data, name,
            clock_res_get clock_time_get fd_fdstat_get fd_read fd_seek fd_write poll_oneoff
            proc_exit random_get sched_yield
        )?;
    }
    Ok(())
}
