use std::path::{Path, PathBuf};
use std::ptr;

use freetype::freetype as ft;

use crate::library::FontData;

pub struct SystemFonts {
    fontconfig: Option<fontconfig::Fontconfig>,
    scanned: Option<Vec<PathBuf>>,
    given: Vec<(PathBuf, u32)>,
}

impl SystemFonts {
    pub fn new() -> Self {
        Self {
            fontconfig: fontconfig::Fontconfig::open(),
            scanned: None,
            given: Vec::new(),
        }
    }

    pub fn lookup(&mut self, character: char) -> Option<FontData> {
        let (path, index) = self
            .candidates(character)
            .into_iter()
            .find(|candidate| !self.given.contains(candidate))?;
        let font = read(&path, index)?;
        self.given.push((path, index));
        Some(font)
    }

    pub fn candidates(&mut self, character: char) -> Vec<(PathBuf, u32)> {
        match &self.fontconfig {
            Some(fontconfig) => fontconfig.covering(character),
            None => self.scan(character),
        }
    }

    fn scan(&mut self, character: char) -> Vec<(PathBuf, u32)> {
        let files = self.scanned.get_or_insert_with(font_files);
        let mut library = ptr::null_mut();
        if unsafe { ft::FT_Init_FreeType(&mut library) } != 0 {
            return Vec::new();
        }
        let found = files
            .iter()
            .find_map(|path| face_covering(library, path, character));
        unsafe {
            ft::FT_Done_FreeType(library);
        }
        found.into_iter().collect()
    }
}

impl Default for SystemFonts {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read(path: &Path, index: u32) -> Option<FontData> {
    Some(FontData::new(std::fs::read(path).ok()?, index))
}

pub fn fallback() -> impl FnMut(char) -> Option<FontData> {
    let mut fonts = SystemFonts::new();
    move |character| fonts.lookup(character)
}

fn face_covering(library: ft::FT_Library, path: &Path, character: char) -> Option<(PathBuf, u32)> {
    let name = std::ffi::CString::new(path.to_str()?).ok()?;
    let mut index = 0;
    loop {
        let mut face = ptr::null_mut();
        if unsafe { ft::FT_New_Face(library, name.as_ptr(), index, &mut face) } != 0 {
            return None;
        }
        let (faces, usable) = unsafe {
            let flags = (*face).face_flags;
            let scalable = flags & ft::FT_FACE_FLAG_SCALABLE as ft::FT_Long != 0;
            let color = flags & ft::FT_FACE_FLAG_COLOR as ft::FT_Long != 0;
            let covers = ft::FT_Get_Char_Index(face, character as ft::FT_ULong) != 0;
            let faces = (*face).num_faces;
            ft::FT_Done_Face(face);
            (faces, scalable && !color && covers)
        };
        if usable {
            return Some((path.to_path_buf(), index as u32));
        }
        index += 1;
        if index >= faces {
            return None;
        }
    }
}

fn font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if cfg!(target_os = "android") {
        directories.push(PathBuf::from("/system/fonts"));
    } else if cfg!(target_os = "macos") {
        directories.push(PathBuf::from("/System/Library/Fonts"));
        directories.push(PathBuf::from("/Library/Fonts"));
    } else if cfg!(target_os = "windows") {
        let root =
            std::env::var_os("WINDIR").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
        directories.push(root.join("Fonts"));
    } else {
        directories.push(PathBuf::from("/usr/share/fonts"));
        directories.push(PathBuf::from("/usr/local/share/fonts"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.push(home.join(".local/share/fonts"));
        directories.push(home.join(".fonts"));
        directories.push(home.join("Library/Fonts"));
    }
    directories
}

fn font_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = font_directories();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase);
            if matches!(extension.as_deref(), Some("ttf" | "otf" | "ttc" | "otc")) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
mod fontconfig {
    use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
    use std::path::PathBuf;
    use std::ptr;

    use libloading::{Library, Symbol};

    const MATCH_PATTERN: c_int = 0;
    const RESULT_MATCH: c_int = 0;
    const CANDIDATES: usize = 8;

    type Config = c_void;
    type Pattern = c_void;
    type CharSet = c_void;

    #[repr(C)]
    struct FontSet {
        count: c_int,
        size: c_int,
        fonts: *mut *mut Pattern,
    }

    pub struct Fontconfig {
        library: Library,
        config: *mut Config,
    }

    impl Fontconfig {
        pub fn open() -> Option<Self> {
            let library = ["libfontconfig.so.1", "libfontconfig.so"]
                .into_iter()
                .find_map(|name| unsafe { Library::new(name).ok() })?;
            let config = unsafe {
                let init: Symbol<unsafe extern "C" fn() -> *mut Config> =
                    library.get(b"FcInitLoadConfigAndFonts\0").ok()?;
                init()
            };
            (!config.is_null()).then_some(Self { library, config })
        }

        pub fn covering(&self, character: char) -> Vec<(PathBuf, u32)> {
            unsafe { self.sorted(character) }.unwrap_or_default()
        }

        unsafe fn sorted(&self, character: char) -> Option<Vec<(PathBuf, u32)>> {
            unsafe {
                let library = &self.library;
                let pattern_create: Symbol<unsafe extern "C" fn() -> *mut Pattern> =
                    library.get(b"FcPatternCreate\0").ok()?;
                let pattern_destroy: Symbol<unsafe extern "C" fn(*mut Pattern)> =
                    library.get(b"FcPatternDestroy\0").ok()?;
                let charset_create: Symbol<unsafe extern "C" fn() -> *mut CharSet> =
                    library.get(b"FcCharSetCreate\0").ok()?;
                let charset_destroy: Symbol<unsafe extern "C" fn(*mut CharSet)> =
                    library.get(b"FcCharSetDestroy\0").ok()?;
                let charset_add: Symbol<unsafe extern "C" fn(*mut CharSet, c_uint) -> c_int> =
                    library.get(b"FcCharSetAddChar\0").ok()?;
                let charset_has: Symbol<unsafe extern "C" fn(*const CharSet, c_uint) -> c_int> =
                    library.get(b"FcCharSetHasChar\0").ok()?;
                let add_charset: Symbol<
                    unsafe extern "C" fn(*mut Pattern, *const c_char, *const CharSet) -> c_int,
                > = library.get(b"FcPatternAddCharSet\0").ok()?;
                let substitute: Symbol<
                    unsafe extern "C" fn(*mut Config, *mut Pattern, c_int) -> c_int,
                > = library.get(b"FcConfigSubstitute\0").ok()?;
                let default_substitute: Symbol<unsafe extern "C" fn(*mut Pattern)> =
                    library.get(b"FcDefaultSubstitute\0").ok()?;
                let sort: Symbol<
                    unsafe extern "C" fn(
                        *mut Config,
                        *mut Pattern,
                        c_int,
                        *mut *mut CharSet,
                        *mut c_int,
                    ) -> *mut FontSet,
                > = library.get(b"FcFontSort\0").ok()?;
                let set_destroy: Symbol<unsafe extern "C" fn(*mut FontSet)> =
                    library.get(b"FcFontSetDestroy\0").ok()?;
                let get_charset: Symbol<
                    unsafe extern "C" fn(
                        *const Pattern,
                        *const c_char,
                        c_int,
                        *mut *mut CharSet,
                    ) -> c_int,
                > = library.get(b"FcPatternGetCharSet\0").ok()?;
                let get_string: Symbol<
                    unsafe extern "C" fn(
                        *const Pattern,
                        *const c_char,
                        c_int,
                        *mut *mut u8,
                    ) -> c_int,
                > = library.get(b"FcPatternGetString\0").ok()?;
                let get_integer: Symbol<
                    unsafe extern "C" fn(*const Pattern, *const c_char, c_int, *mut c_int) -> c_int,
                > = library.get(b"FcPatternGetInteger\0").ok()?;
                let get_bool: Symbol<
                    unsafe extern "C" fn(*const Pattern, *const c_char, c_int, *mut c_int) -> c_int,
                > = library.get(b"FcPatternGetBool\0").ok()?;

                let codepoint = character as c_uint;
                let pattern = pattern_create();
                let charset = charset_create();
                charset_add(charset, codepoint);
                add_charset(pattern, c"charset".as_ptr(), charset);
                substitute(self.config, pattern, MATCH_PATTERN);
                default_substitute(pattern);
                let mut result = 0;
                let set = sort(self.config, pattern, 1, ptr::null_mut(), &mut result);
                pattern_destroy(pattern);
                charset_destroy(charset);
                if set.is_null() {
                    return None;
                }
                let mut found = Vec::new();
                let fonts = &*set;
                for at in 0..fonts.count.max(0) as usize {
                    if found.len() >= CANDIDATES {
                        break;
                    }
                    let font = *fonts.fonts.add(at);
                    let mut covered = ptr::null_mut();
                    if get_charset(font, c"charset".as_ptr(), 0, &mut covered) != RESULT_MATCH
                        || charset_has(covered, codepoint) == 0
                    {
                        continue;
                    }
                    let mut scalable = 1;
                    let mut color = 0;
                    get_bool(font, c"scalable".as_ptr(), 0, &mut scalable);
                    get_bool(font, c"color".as_ptr(), 0, &mut color);
                    if scalable == 0 || color != 0 {
                        continue;
                    }
                    let mut file = ptr::null_mut();
                    if get_string(font, c"file".as_ptr(), 0, &mut file) != RESULT_MATCH
                        || file.is_null()
                    {
                        continue;
                    }
                    let mut index = 0;
                    get_integer(font, c"index".as_ptr(), 0, &mut index);
                    let path = CStr::from_ptr(file.cast()).to_string_lossy().into_owned();
                    found.push((PathBuf::from(path), index.max(0) as u32));
                }
                set_destroy(set);
                Some(found)
            }
        }
    }
}

#[cfg(not(all(unix, not(target_os = "android"), not(target_os = "macos"))))]
mod fontconfig {
    use std::path::PathBuf;

    pub struct Fontconfig;

    impl Fontconfig {
        pub fn open() -> Option<Self> {
            None
        }

        pub fn covering(&self, _character: char) -> Vec<(PathBuf, u32)> {
            Vec::new()
        }
    }
}
