#[cfg(target_arch = "wasm32")]
pub struct Event(pub(crate) web_sys::Event);

#[cfg(not(target_arch = "wasm32"))]
pub struct Event(Never);

#[cfg(not(target_arch = "wasm32"))]
enum Never {}

#[cfg(target_arch = "wasm32")]
impl Event {
    pub fn raw(&self) -> &web_sys::Event {
        &self.0
    }

    pub fn key(&self) -> Option<String> {
        use wasm_bindgen::JsCast;
        self.0
            .dyn_ref::<web_sys::KeyboardEvent>()
            .map(web_sys::KeyboardEvent::key)
    }

    pub fn target_value(&self) -> Option<String> {
        let target = self.0.target()?;
        js_sys::Reflect::get(&target, &"value".into())
            .ok()?
            .as_string()
    }

    pub fn target_checked(&self) -> Option<bool> {
        let target = self.0.target()?;
        js_sys::Reflect::get(&target, &"checked".into())
            .ok()?
            .as_bool()
    }

    pub fn prevent_default(&self) {
        self.0.prevent_default();
    }

    pub fn stop_propagation(&self) {
        self.0.stop_propagation();
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Event {
    pub fn key(&self) -> Option<String> {
        match self.0 {}
    }

    pub fn target_value(&self) -> Option<String> {
        match self.0 {}
    }

    pub fn target_checked(&self) -> Option<bool> {
        match self.0 {}
    }

    pub fn prevent_default(&self) {
        match self.0 {}
    }

    pub fn stop_propagation(&self) {
        match self.0 {}
    }
}
