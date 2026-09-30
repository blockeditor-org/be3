use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone)]
pub enum FontBytes {
    Static(&'static [u8]),
    Shared(Arc<[u8]>),
}

impl Deref for FontBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Static(bytes) => bytes,
            Self::Shared(bytes) => bytes,
        }
    }
}

impl From<Vec<u8>> for FontBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Shared(bytes.into())
    }
}

#[derive(Clone)]
pub struct FontData {
    pub bytes: FontBytes,
    pub index: u32,
}

impl FontData {
    pub const fn from_static(bytes: &'static [u8]) -> Self {
        Self {
            bytes: FontBytes::Static(bytes),
            index: 0,
        }
    }

    pub fn new(bytes: impl Into<FontBytes>, index: u32) -> Self {
        Self {
            bytes: bytes.into(),
            index,
        }
    }

    pub fn same(&self, other: &FontData) -> bool {
        self.index == other.index
            && self.bytes.len() == other.bytes.len()
            && (std::ptr::eq(self.bytes.as_ptr(), other.bytes.as_ptr())
                || *self.bytes == *other.bytes)
    }
}

impl fmt::Debug for FontData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "FontData({} bytes, face {})",
            self.bytes.len(),
            self.index
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct FontSources {
    pub proportional: Vec<FontData>,
    pub monospace: Vec<FontData>,
    pub fallback: Vec<FontData>,
    pub icons: Vec<FontData>,
}

impl FontSources {
    pub fn bundled() -> Self {
        Self {
            proportional: vec![FontData::from_static(UBUNTU_LIGHT)],
            monospace: vec![FontData::from_static(HACK_REGULAR)],
            fallback: vec![FontData::from_static(NOTO_EMOJI_REGULAR)],
            icons: vec![FontData::from_static(ICONS_FONT)],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.proportional.is_empty()
            && self.monospace.is_empty()
            && self.fallback.is_empty()
            && self.icons.is_empty()
    }

    pub fn covers(&self, data: &FontData) -> bool {
        [
            &self.proportional,
            &self.monospace,
            &self.fallback,
            &self.icons,
        ]
        .into_iter()
        .flatten()
        .any(|held| held.same(data))
    }
}

type Resolve = Box<dyn FnMut(char) -> Option<FontData>>;

struct Library {
    sources: FontSources,
    generation: u64,
    resolve: Option<Resolve>,
    asked: HashSet<char>,
}

#[derive(Clone)]
pub struct FontLibrary(Rc<RefCell<Library>>);

impl FontLibrary {
    pub fn new(sources: FontSources) -> Self {
        Self(Rc::new(RefCell::new(Library {
            sources,
            generation: 0,
            resolve: None,
            asked: HashSet::new(),
        })))
    }

    pub fn bundled() -> Self {
        Self::new(FontSources::bundled())
    }

    pub fn with_fallback(self, resolve: impl FnMut(char) -> Option<FontData> + 'static) -> Self {
        self.0.borrow_mut().resolve = Some(Box::new(resolve));
        self
    }

    pub fn generation(&self) -> u64 {
        self.0.borrow().generation
    }

    pub fn sources(&self) -> FontSources {
        self.0.borrow().sources.clone()
    }

    pub fn replace(&self, sources: FontSources) {
        let mut library = self.0.borrow_mut();
        library.sources = sources;
        library.asked.clear();
        library.generation += 1;
    }

    pub fn add_fallback(&self, fonts: Vec<FontData>) {
        let mut library = self.0.borrow_mut();
        let mut added = false;
        for font in fonts {
            if !library.sources.covers(&font) {
                library.sources.fallback.push(font);
                added = true;
            }
        }
        if added {
            library.generation += 1;
        }
    }

    pub(crate) fn fallback_count(&self) -> usize {
        self.0.borrow().sources.fallback.len()
    }

    pub(crate) fn fallback_from(&self, start: usize) -> Vec<FontData> {
        self.0
            .borrow()
            .sources
            .fallback
            .get(start..)
            .map(<[FontData]>::to_vec)
            .unwrap_or_default()
    }

    pub(crate) fn missing(&self, character: char) -> bool {
        let mut library = self.0.borrow_mut();
        if !library.asked.insert(character) {
            return false;
        }
        let Some(resolve) = library.resolve.as_mut() else {
            return false;
        };
        let Some(font) = resolve(character) else {
            return false;
        };
        if library.sources.covers(&font) {
            return false;
        }
        library.sources.fallback.push(font);
        true
    }
}

impl Default for FontLibrary {
    fn default() -> Self {
        Self::bundled()
    }
}

pub const ICONS_FONT: &[u8] = include_bytes!("../assets/icons/MaterialSymbolsRounded-Filled.ttf");
const UBUNTU_LIGHT: &[u8] = include_bytes!("../assets/fonts/Ubuntu-Light.ttf");
const HACK_REGULAR: &[u8] = include_bytes!("../assets/fonts/Hack-Regular.ttf");
const NOTO_EMOJI_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf");
