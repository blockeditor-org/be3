//! The keys a `KeepAlive` keeps its subtrees by (`beui-view`'s `Kept`), and which keys it keeps.

use std::any::{Any, TypeId};
use std::hash::{BuildHasher, Hash, Hasher};
use std::rc::Rc;

use crate::component::MissingProp;
use crate::prop::Prop;
use beui_reactive::{Get, Memo};

/// A key with its type forgotten, so one `KeepAlive` keeps keys of any type.
#[derive(Clone)]
pub struct AnyKey {
    value: Rc<dyn Any>,
    hash: u64,
    equals: fn(&dyn Any, &dyn Any) -> bool,
}

impl AnyKey {
    pub fn new<K: Hash + Eq + 'static>(key: K) -> Self {
        let mut hasher = be_lib_stable_hash::State::default().build_hasher();
        TypeId::of::<K>().hash(&mut hasher);
        key.hash(&mut hasher);
        Self {
            value: Rc::new(key),
            hash: hasher.finish(),
            equals: |a, b| match (a.downcast_ref::<K>(), b.downcast_ref::<K>()) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        }
    }
}

impl PartialEq for AnyKey {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash && (self.equals)(&*self.value, &*other.value)
    }
}

impl Eq for AnyKey {}

impl Hash for AnyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

/// What a `Kept` is known by in its `KeepAlive`: its `key`, or, without one, nothing, so a
/// `KeepAlive` keeps one unkeyed subtree.
pub trait KeptKey: 'static {
    fn into_any_key(self) -> AnyKey;
}

impl KeptKey for MissingProp {
    fn into_any_key(self) -> AnyKey {
        AnyKey::new(())
    }
}

impl<K: Hash + Eq + 'static> KeptKey for K {
    fn into_any_key(self) -> AnyKey {
        AnyKey::new(self)
    }
}

/// Which keys a `KeepAlive` keeps while nothing shows them: all of them, or the keys a list
/// names (`keep={open_tabs}`), a `Prop` or a `Memo`, read as it changes, so a key dropped from
/// it goes for good.
pub trait KeepSet: 'static {
    /// The keys kept, read tracked, or `None` when every key is.
    fn keys(&self) -> Option<Vec<AnyKey>>;
}

impl KeepSet for MissingProp {
    fn keys(&self) -> Option<Vec<AnyKey>> {
        None
    }
}

impl<K: Hash + Eq + Clone + 'static> KeepSet for Prop<Vec<K>> {
    fn keys(&self) -> Option<Vec<AnyKey>> {
        Some(self.with(|keys| keys.iter().cloned().map(AnyKey::new).collect()))
    }
}

// A bare generic prop takes the value as written, so a memo arrives as a `Memo`, not a `Prop`.
impl<K: Hash + Eq + Clone + 'static> KeepSet for Memo<Vec<K>> {
    fn keys(&self) -> Option<Vec<AnyKey>> {
        Some(self.with(|keys: &Vec<K>| keys.iter().cloned().map(AnyKey::new).collect()))
    }
}

/// Whether the kept subtree around the caller (a `Kept`, in `beui-view`) is shown, and every
/// kept subtree around that: `None` outside any. What a kept subtree holds outside itself while
/// hidden (a dock it joined) lets go by it, and takes back when it shows again.
pub fn kept_shown() -> Option<Memo<bool>> {
    beui_reactive::use_context::<KeptShown>().map(|KeptShown(shown)| shown)
}

/// What a `Kept` provides its subtree for `kept_shown()`.
#[doc(hidden)]
pub fn provide_kept_shown(shown: Memo<bool>) {
    beui_reactive::provide_context(KeptShown(shown));
}

#[derive(Clone, Copy)]
struct KeptShown(Memo<bool>);
