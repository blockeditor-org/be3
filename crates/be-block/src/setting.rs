use std::rc::Rc;

use be_model::{Edit, FieldRef, ObjectId, Register};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsLayer {
    EveryDevice,
    ThisDevice,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layers {
    pub every_device: Option<Uuid>,
    pub this_device: Option<Uuid>,
}

impl Layers {
    pub fn top(self) -> Option<Uuid> {
        self.this_device.or(self.every_device)
    }

    pub fn below(self, block: Uuid) -> Vec<Uuid> {
        match self.this_device == Some(block) {
            true => self.every_device.into_iter().collect(),
            false => Vec::new(),
        }
    }

    pub fn block(self, layer: SettingsLayer) -> Option<Uuid> {
        match layer {
            SettingsLayer::EveryDevice => self.every_device,
            SettingsLayer::ThisDevice => self.this_device,
        }
    }

    pub fn bottom_up(self) -> Vec<Uuid> {
        self.every_device
            .into_iter()
            .chain(self.this_device)
            .collect()
    }
}

type Get<R, V> = dyn Fn(&R) -> Option<V>;
type Set<R, V> = dyn Fn(&R, Option<V>) -> Edit;

pub struct Setting<R, V> {
    key: Rc<str>,
    name: Rc<str>,
    this_device_only: bool,
    default: Option<V>,
    get: Rc<Get<R, V>>,
    set: Rc<Set<R, V>>,
}

impl<R, V: Clone> Clone for Setting<R, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            name: self.name.clone(),
            this_device_only: self.this_device_only,
            default: self.default.clone(),
            get: self.get.clone(),
            set: self.set.clone(),
        }
    }
}

impl<R, V: Clone> Setting<R, V> {
    pub fn new(
        key: impl Into<String>,
        name: impl Into<String>,
        get: impl Fn(&R) -> Option<V> + 'static,
        set: impl Fn(&R, Option<V>) -> Edit + 'static,
    ) -> Self {
        Self {
            key: key.into().into(),
            name: name.into().into(),
            this_device_only: false,
            default: None,
            get: Rc::new(get),
            set: Rc::new(set),
        }
    }

    pub fn field(
        key: impl Into<String>,
        name: impl Into<String>,
        field: FieldRef<R, Option<V>>,
        get: impl Fn(&R) -> Option<V> + 'static,
    ) -> Self
    where
        R: 'static,
        V: 'static,
        Option<V>: Register,
    {
        Self::new(key, name, get, move |_, value| {
            field.set(ObjectId::ROOT, &value).into()
        })
    }

    pub fn with_default(mut self, default: Option<V>) -> Self {
        self.default = default;
        self
    }

    pub fn only_on_this_device(mut self, only: bool) -> Self {
        self.this_device_only = only;
        self
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_this_device_only(&self) -> bool {
        self.this_device_only
    }

    pub fn default_value(&self) -> Option<V> {
        self.default.clone()
    }

    pub fn get(&self, settings: &R) -> Option<V> {
        (self.get)(settings)
    }

    pub fn set(&self, own: &R, value: Option<V>) -> Edit {
        (self.set)(own, value)
    }

    pub fn kept(&self, this_device: &R) -> bool {
        self.this_device_only || self.get(this_device).is_some()
    }

    pub fn layer(&self, this_device: &R) -> SettingsLayer {
        match self.kept(this_device) {
            true => SettingsLayer::ThisDevice,
            false => SettingsLayer::EveryDevice,
        }
    }
}
