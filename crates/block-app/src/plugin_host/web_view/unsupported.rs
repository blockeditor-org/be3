use block_plugin_api::WebViewEvent;

use crate::host::WakingSender;

use super::Bounds;

pub(super) enum WebView {}

impl WebView {
    pub(super) fn new(_url: &str, _events: &WakingSender<WebViewEvent>) -> Result<Self, String> {
        Err("The embedded browser is not supported on this platform.".to_owned())
    }

    pub(super) fn load_url(&self, _url: &str) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn reload(&self) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn set_bounds(&self, _bounds: Bounds) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn set_visible(&self, _visible: bool) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn focus_parent(&self) -> Result<(), String> {
        match *self {}
    }
}
