use std::cell::RefCell;
use std::rc::Rc;

use block_editor_plugin::be_block::{BrowserTab, BrowserTabContent, Edit, HistoryItem};
use block_editor_plugin::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_memo, create_signal,
};
use block_editor_plugin::{ContentProjection, Editor, WebViewEvent};

#[derive(Default)]
struct Navigation {
    opened: bool,
    current: Option<String>,
    synchronized: Option<(usize, String)>,
    programmatic: Option<String>,
    natural: Option<String>,
}

pub(crate) struct Session {
    editor: Editor,
    tab: Rc<ContentProjection<BrowserTabContent>>,
    navigation: RefCell<Navigation>,
    address: ReadSignal<String>,
    set_address: WriteSignal<String>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
}

impl Session {
    pub(crate) fn new(editor: &Editor) -> Rc<Self> {
        let (address, set_address) = create_signal("about:blank".to_owned());
        let (error, set_error) = create_signal(None::<String>);
        let session = Rc::new(Self {
            editor: editor.clone(),
            tab: editor.block_content::<BrowserTabContent>(),
            navigation: RefCell::default(),
            address,
            set_address,
            error,
            set_error,
        });
        let frame = Rc::clone(&session);
        editor.each_frame(move || frame.poll());
        session
    }

    pub(crate) fn address(&self) -> ReadSignal<String> {
        self.address.clone()
    }

    pub(crate) fn set_address(&self, address: String) {
        self.set_address.set(address);
    }

    pub(crate) fn error(&self) -> ReadSignal<Option<String>> {
        self.error.clone()
    }

    pub(crate) fn back(&self) -> Memo<Option<usize>> {
        let index = self.tab.project(|tab| tab.root().index());
        create_memo(move || index.get().checked_sub(1))
    }

    pub(crate) fn forward(&self) -> Memo<Option<usize>> {
        let step = self.tab.project(|tab| {
            let tab = tab.root();
            (tab.index(), tab.can_go_forward())
        });
        create_memo(move || {
            let (index, can) = step.get();
            can.then_some(index + 1)
        })
    }

    pub(crate) fn go(&self, index: usize) {
        self.edit(|tab| tab.go(index));
    }

    pub(crate) fn reload(&self) {
        self.editor.host().reload_web_view();
    }

    pub(crate) fn focus_app(&self) {
        self.editor.host().focus_app();
    }

    pub(crate) fn navigate(&self, address: &str) {
        let url = browser_url(address);
        self.set_address.set(url.clone());
        self.push(url);
    }

    fn poll(self: &Rc<Self>) {
        self.process_events();
        self.open();
        self.synchronize();
        self.place();
    }

    fn place(&self) {
        let rect = self.error.with(Option::is_none).then(|| {
            let rect = self.editor.content_rect();
            rect.is_positive().then_some(rect)
        });
        self.editor.place_web_view(rect.flatten());
    }

    fn open(&self) {
        let mut navigation = self.navigation.borrow_mut();
        if navigation.opened {
            return;
        }
        let Some((index, url)) = self.tab.read(|tab| {
            let tab = tab.root();
            (tab.index(), tab.current().url)
        }) else {
            return;
        };
        navigation.opened = true;
        navigation.current = Some(url.clone());
        navigation.programmatic = Some(url.clone());
        navigation.synchronized = Some((index, url.clone()));
        drop(navigation);
        self.set_address.set(url.clone());
        self.editor.host().open_web_view(url);
    }

    fn process_events(&self) {
        for event in self.editor.host().take_web_view_events() {
            match event {
                WebViewEvent::Navigate(url) => self.navigation_started(url),
                WebViewEvent::Finished(url) => self.navigation_finished(url),
                WebViewEvent::Push(url) => self.push_item(url),
                WebViewEvent::Replace(url) => self.replace(url),
                WebViewEvent::Title(title) => self.title_changed(title),
                WebViewEvent::History(delta) => self.traverse_history(delta as isize),
                WebViewEvent::NewWindow(url) => self.push(url),
                WebViewEvent::Address(url) => self.navigation.borrow_mut().current = Some(url),
                WebViewEvent::Failed(error) => self.set_error.set(Some(error)),
            }
        }
    }

    fn edit(&self, make: impl FnOnce(&BrowserTab) -> Edit) {
        let edit = self.tab.read(|tab| make(&tab.root()));
        if let Some(edit) = edit.filter(|edit| !edit.0.is_empty()) {
            self.tab.operate(edit);
        }
    }

    fn push_item(&self, url: String) {
        let item = self.item_with_url(url);
        self.edit(|tab| tab.push(&item));
    }

    fn push(&self, url: String) {
        let item = HistoryItem::new(url, "");
        self.edit(|tab| tab.push(&item));
    }

    fn replace(&self, url: String) {
        let item = self.item_with_url(url);
        self.edit(|tab| tab.replace(&item));
    }

    fn navigation_started(&self, url: String) {
        let mut navigation = self.navigation.borrow_mut();
        if let Some(expected) = &navigation.programmatic {
            if expected == &url {
                return;
            }
            navigation.programmatic = Some(url.clone());
            drop(navigation);
            self.replace(url);
            return;
        }
        if let Some(expected) = &navigation.natural {
            if expected == &url {
                return;
            }
            navigation.natural = Some(url.clone());
            drop(navigation);
            self.replace(url);
            return;
        }
        navigation.natural = Some(url.clone());
        drop(navigation);
        self.push(url);
    }

    fn navigation_finished(&self, url: String) {
        let mut navigation = self.navigation.borrow_mut();
        navigation.programmatic = None;
        navigation.natural = None;
        drop(navigation);
        self.set_address.set(url.clone());
        let current = self.tab.read(|tab| tab.root().current().url);
        if current.as_deref().is_some_and(|current| current != url) {
            self.replace(url);
        }
    }

    fn title_changed(&self, title: String) {
        let Some(shown) = self.tab.read(|tab| tab.root().current().url) else {
            return;
        };
        let url = self.navigation.borrow().current.clone().unwrap_or(shown);
        let item = HistoryItem::new(url, title);
        self.edit(|tab| tab.replace(&item));
    }

    fn item_with_url(&self, url: String) -> HistoryItem {
        let title = self
            .tab
            .read(|tab| tab.root().current().title)
            .unwrap_or_default();
        HistoryItem::new(url, title)
    }

    fn traverse_history(&self, delta: isize) {
        let Some((index, length)) = self.tab.read(|tab| {
            let tab = tab.root();
            (tab.index(), tab.history.len())
        }) else {
            return;
        };
        let Some(index) = index.checked_add_signed(delta) else {
            return;
        };
        if index < length {
            self.go(index);
        }
    }

    fn synchronize(&self) {
        let Some(selected) = self.tab.read(|tab| {
            let tab = tab.root();
            (tab.index(), tab.current().url)
        }) else {
            return;
        };
        let mut navigation = self.navigation.borrow_mut();
        if navigation.synchronized.as_ref() == Some(&selected) {
            return;
        }
        let url = selected.1.clone();
        let natural = navigation.natural.as_deref() == Some(url.as_str());
        let load = !natural && navigation.current.as_deref() != Some(url.as_str());
        if load {
            navigation.programmatic = Some(url.clone());
        }
        navigation.synchronized = Some(selected);
        drop(navigation);
        self.set_address.set(url.clone());
        if load {
            self.editor.host().load_web_view(url);
        }
    }
}

fn browser_url(address: &str) -> String {
    let address = address.trim();
    if address.contains("://")
        || address.starts_with("about:")
        || address.starts_with("data:")
        || address.starts_with("file:")
    {
        address.to_owned()
    } else if address.starts_with("localhost") || address.starts_with("127.0.0.1") {
        format!("http://{address}")
    } else {
        format!("https://{address}")
    }
}
