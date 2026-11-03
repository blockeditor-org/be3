use be_sdk_api::{
    HostWindow, HostWindowId, MenuAsk, Picture, Rect, Vec2, WindowKind, WindowOrigin,
};

#[derive(Clone, Debug, PartialEq)]
pub struct KnownWindow {
    pub id: HostWindowId,
    pub kind: WindowKind,
    pub title: String,
    pub program_id: String,
    pub name: String,
    pub parent: Option<HostWindowId>,
    pub modal: bool,
    pub size: Vec2,
    pub min_size: Vec2,
    pub fullscreen: Option<Rect>,
    pub responding: bool,
    pub icon: Option<Picture>,
    pub raised: u64,
    pub rung: u64,
    pub origin: WindowOrigin,
    pub note: Option<String>,
    pub menu: Option<MenuAsk>,
}

impl Default for KnownWindow {
    fn default() -> Self {
        Self {
            id: HostWindowId(0),
            kind: WindowKind::Window,
            title: String::new(),
            program_id: String::new(),
            name: String::new(),
            parent: None,
            modal: false,
            size: Vec2::ZERO,
            min_size: Vec2::ZERO,
            fullscreen: None,
            responding: true,
            icon: None,
            raised: 0,
            rung: 0,
            origin: WindowOrigin::default(),
            note: None,
            menu: None,
        }
    }
}

impl KnownWindow {
    pub fn listed(&self) -> HostWindow {
        HostWindow {
            id: self.id,
            kind: self.kind,
            parent: self.parent,
            modal: self.modal,
            program_id: self.program_id.clone(),
            origin: self.origin.clone(),
        }
    }
}
