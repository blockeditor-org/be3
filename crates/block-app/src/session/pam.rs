use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;

use super::lock::Verdict;

pub(crate) const SERVICE: &str = "block-app";
const FALLBACK_SERVICE: &str = "login";
const LIBRARY: &CStr = c"libpam.so.0";

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_PERM_DENIED: c_int = 6;
const PAM_AUTH_ERR: c_int = 7;
const PAM_CRED_INSUFFICIENT: c_int = 8;
const PAM_USER_UNKNOWN: c_int = 10;
const PAM_MAXTRIES: c_int = 11;
const PAM_CONV_ERR: c_int = 19;
const PAM_DISALLOW_NULL_AUTHTOK: c_int = 0x0001;

const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_ERROR_MSG: c_int = 3;
const PAM_TEXT_INFO: c_int = 4;

#[repr(C)]
struct Message {
    style: c_int,
    text: *const c_char,
}

#[repr(C)]
struct Response {
    text: *mut c_char,
    code: c_int,
}

type Converse = unsafe extern "C" fn(
    count: c_int,
    messages: *mut *const Message,
    responses: *mut *mut Response,
    data: *mut c_void,
) -> c_int;

#[repr(C)]
struct Conversation {
    converse: Converse,
    data: *mut c_void,
}

type Start = unsafe extern "C" fn(
    service: *const c_char,
    user: *const c_char,
    conversation: *const Conversation,
    handle: *mut *mut c_void,
) -> c_int;
type Authenticate = unsafe extern "C" fn(handle: *mut c_void, flags: c_int) -> c_int;
type End = unsafe extern "C" fn(handle: *mut c_void, status: c_int) -> c_int;
type Describe = unsafe extern "C" fn(handle: *mut c_void, status: c_int) -> *const c_char;

struct Library {
    handle: *mut c_void,
    start: Start,
    authenticate: Authenticate,
    end: End,
    describe: Describe,
}

impl Library {
    fn open() -> Result<Self, String> {
        let handle = unsafe { libc::dlopen(LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if handle.is_null() {
            return Err("PAM (libpam.so.0) is not installed".to_owned());
        }
        let symbol = |name: &CStr| {
            let found = unsafe { libc::dlsym(handle, name.as_ptr()) };
            match found.is_null() {
                true => Err(format!("libpam has no {}", name.to_string_lossy())),
                false => Ok(found),
            }
        };
        let opened = (|| {
            Ok(Self {
                handle,
                start: unsafe { std::mem::transmute::<*mut c_void, Start>(symbol(c"pam_start")?) },
                authenticate: unsafe {
                    std::mem::transmute::<*mut c_void, Authenticate>(symbol(c"pam_authenticate")?)
                },
                end: unsafe { std::mem::transmute::<*mut c_void, End>(symbol(c"pam_end")?) },
                describe: unsafe {
                    std::mem::transmute::<*mut c_void, Describe>(symbol(c"pam_strerror")?)
                },
            })
        })();
        if opened.is_err() {
            unsafe { libc::dlclose(handle) };
        }
        opened
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe { libc::dlclose(self.handle) };
    }
}

struct Answers {
    user: CString,
    password: CString,
    said: Vec<String>,
}

impl Drop for Answers {
    fn drop(&mut self) {
        let bytes = std::mem::take(&mut self.password).into_bytes_with_nul();
        let mut bytes = bytes;
        for byte in bytes.iter_mut() {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

pub(crate) fn service() -> &'static str {
    let installed = ["/etc/pam.d", "/usr/lib/pam.d", "/usr/local/etc/pam.d"]
        .iter()
        .any(|dir| Path::new(dir).join(SERVICE).is_file());
    match installed {
        true => SERVICE,
        false => FALLBACK_SERVICE,
    }
}

pub(crate) fn authenticate(service: &str, user: &str, password: &str) -> Verdict {
    let library = match Library::open() {
        Ok(library) => library,
        Err(problem) => return Verdict::Failed(problem),
    };
    let (Ok(service), Ok(user_name), Ok(secret)) = (
        CString::new(service),
        CString::new(user),
        CString::new(password),
    ) else {
        return Verdict::Denied("That password is not right.".to_owned());
    };
    let mut answers = Box::new(Answers {
        user: user_name.clone(),
        password: secret,
        said: Vec::new(),
    });
    let conversation = Conversation {
        converse,
        data: std::ptr::from_mut::<Answers>(&mut answers).cast(),
    };
    let mut handle = std::ptr::null_mut();
    let started =
        unsafe { (library.start)(service.as_ptr(), user_name.as_ptr(), &conversation, &mut handle) };
    if started != PAM_SUCCESS || handle.is_null() {
        return Verdict::Failed(format!("PAM did not start ({started})"));
    }
    let status = unsafe { (library.authenticate)(handle, PAM_DISALLOW_NULL_AUTHTOK) };
    let described = unsafe { (library.describe)(handle, status) };
    let described = match described.is_null() {
        true => format!("PAM error {status}"),
        false => unsafe { CStr::from_ptr(described) }
            .to_string_lossy()
            .into_owned(),
    };
    unsafe { (library.end)(handle, status) };
    let said = answers.said.join(" ");
    match status {
        PAM_SUCCESS => Verdict::Accepted,
        PAM_AUTH_ERR | PAM_USER_UNKNOWN | PAM_CRED_INSUFFICIENT | PAM_PERM_DENIED => {
            Verdict::Denied(match said.is_empty() {
                true => "That password is not right.".to_owned(),
                false => said,
            })
        }
        PAM_MAXTRIES => Verdict::Denied("Too many attempts.".to_owned()),
        _ => Verdict::Failed(described),
    }
}

unsafe extern "C" fn converse(
    count: c_int,
    messages: *mut *const Message,
    responses: *mut *mut Response,
    data: *mut c_void,
) -> c_int {
    let Ok(count) = usize::try_from(count) else {
        return PAM_CONV_ERR;
    };
    if count == 0 || messages.is_null() || responses.is_null() || data.is_null() {
        return PAM_CONV_ERR;
    }
    let answers = unsafe { &mut *data.cast::<Answers>() };
    let replies =
        unsafe { libc::calloc(count, std::mem::size_of::<Response>()) }.cast::<Response>();
    if replies.is_null() {
        return PAM_BUF_ERR;
    }
    for index in 0..count {
        let message = unsafe { *messages.add(index) };
        if message.is_null() {
            unsafe { forget(replies, count) };
            return PAM_CONV_ERR;
        }
        let message = unsafe { &*message };
        let reply = match message.style {
            PAM_PROMPT_ECHO_OFF => unsafe { libc::strdup(answers.password.as_ptr()) },
            PAM_PROMPT_ECHO_ON => unsafe { libc::strdup(answers.user.as_ptr()) },
            PAM_ERROR_MSG | PAM_TEXT_INFO => {
                if !message.text.is_null() {
                    let text = unsafe { CStr::from_ptr(message.text) };
                    answers.said.push(text.to_string_lossy().trim().to_owned());
                }
                continue;
            }
            _ => {
                unsafe { forget(replies, count) };
                return PAM_CONV_ERR;
            }
        };
        if reply.is_null() {
            unsafe { forget(replies, count) };
            return PAM_BUF_ERR;
        }
        unsafe { (*replies.add(index)).text = reply };
    }
    unsafe { *responses = replies };
    PAM_SUCCESS
}

unsafe fn forget(replies: *mut Response, count: usize) {
    for index in 0..count {
        let text = unsafe { (*replies.add(index)).text };
        if !text.is_null() {
            let length = unsafe { libc::strlen(text) };
            for at in 0..length {
                unsafe { std::ptr::write_volatile(text.add(at), 0) };
            }
            unsafe { libc::free(text.cast()) };
        }
    }
    unsafe { libc::free(replies.cast()) };
}

pub(crate) fn user() -> Option<(String, String)> {
    let uid = unsafe { libc::getuid() };
    let mut buffer = vec![0 as c_char; 16 * 1024];
    let mut entry = unsafe { std::mem::zeroed::<libc::passwd>() };
    let mut found = std::ptr::null_mut();
    let status = unsafe {
        libc::getpwuid_r(
            uid,
            &mut entry,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut found,
        )
    };
    if status != 0 || found.is_null() || entry.pw_name.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(entry.pw_name) }
        .to_string_lossy()
        .into_owned();
    let full = match entry.pw_gecos.is_null() {
        true => String::new(),
        false => unsafe { CStr::from_ptr(entry.pw_gecos) }
            .to_string_lossy()
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    };
    let shown = match full.is_empty() {
        true => name.clone(),
        false => full,
    };
    Some((name, shown))
}
