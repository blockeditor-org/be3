use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, SIZE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile, STGM_READ,
};
use windows::Win32::System::Threading::GetProcessId;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Programs, IShellItemImageFactory, IShellLinkW,
    KF_FLAG_DEFAULT, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
    SHCreateItemFromParsingName, SHELLEXECUTEINFOW, SHGetKnownFolderPath, SIIGBF_ICONONLY,
    ShellExecuteExW, ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{GUID, Interface as _, PCWSTR};

use super::desktop::property_string;
use super::{Com, from_wide, icons, other, wide};
use crate::command_line::join_arguments;
use crate::{Image, Launch, Program, Shortcut, programs};

pub fn start_menu_programs() -> Vec<Program> {
    let _com = Com::init();
    let folders: Vec<PathBuf> = [FOLDERID_Programs, FOLDERID_CommonPrograms]
        .iter()
        .filter_map(known_folder)
        .collect();
    programs::list(&folders, resolve)
}

// unsafe(ffi)
pub fn program_icon(program: &Program, size: u32) -> Option<Image> {
    let _com = Com::init();
    let path = wide(&program.path);
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None) }.ok()?;
    let size = i32::try_from(size).ok()?;
    let bitmap = unsafe { factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY) }.ok()?;
    icons::from_premultiplied(bitmap)
}

pub fn launch(program: &Program, launch: Option<u64>) -> io::Result<Launch> {
    let process_id = shell_execute(program.path.as_os_str(), None)?;
    Ok(Launch {
        process_id,
        program: Some(program.id.clone()),
        command: program.command(),
        launch,
        target: program.shortcut.target.clone(),
    })
}

pub fn exec(command: &[String], launch: Option<u64>) -> io::Result<Launch> {
    let (program, arguments) = command
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "an empty command"))?;
    let parameters = join_arguments(arguments);
    let process_id = shell_execute(OsStr::new(program), Some(&parameters))?;
    Ok(Launch {
        process_id,
        program: None,
        command: command.to_vec(),
        launch,
        target: Some(PathBuf::from(program)),
    })
}

// unsafe(ffi)
fn shell_execute(file: &OsStr, parameters: Option<&str>) -> io::Result<Option<u32>> {
    let _com = Com::init();
    let verb = wide("open");
    let file = wide(file);
    let parameters = parameters
        .filter(|parameters| !parameters.is_empty())
        .map(wide);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: parameters
            .as_ref()
            .map_or(PCWSTR::null(), |parameters| PCWSTR(parameters.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(other)?;
    if info.hProcess.0.is_null() {
        return Ok(None);
    }
    let process_id = unsafe { GetProcessId(info.hProcess) };
    let _ = unsafe { CloseHandle(info.hProcess) };
    Ok((process_id != 0).then_some(process_id))
}

// unsafe(ffi)
fn known_folder(folder: &GUID) -> Option<PathBuf> {
    let path = unsafe { SHGetKnownFolderPath(folder, KF_FLAG_DEFAULT, None) }.ok()?;
    let text = unsafe { path.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(path.0 as *const core::ffi::c_void)) };
    text.map(PathBuf::from)
}

// unsafe(ffi)
fn resolve(path: &Path) -> Option<Shortcut> {
    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let file: IPersistFile = link.cast().ok()?;
    let path = wide(path);
    unsafe { file.Load(PCWSTR(path.as_ptr()), STGM_READ) }.ok()?;
    let mut buffer = vec![0u16; 32768];
    let target = unsafe { link.GetPath(&mut buffer, std::ptr::null_mut(), 0) }
        .ok()
        .map(|()| from_wide(&buffer))
        .filter(|target| !target.is_empty())
        .map(PathBuf::from);
    buffer.fill(0);
    let arguments = unsafe { link.GetArguments(&mut buffer) }
        .ok()
        .map(|()| from_wide(&buffer))
        .unwrap_or_default();
    buffer.fill(0);
    let description = unsafe { link.GetDescription(&mut buffer) }
        .ok()
        .map(|()| from_wide(&buffer))
        .unwrap_or_default();
    let app_user_model_id = link
        .cast::<IPropertyStore>()
        .ok()
        .and_then(|store| property_string(&store));
    Some(Shortcut {
        target,
        arguments,
        description,
        app_user_model_id,
    })
}
