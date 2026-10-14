use std::cell::RefCell;

use block_editor_beui::be_block::{BrowserTab, BrowserTabContent, Edit, HistoryItem};
use block_editor_beui::beui::reactive::{
    Memo, ReadSignal, Stored, WriteSignal, create_effect, create_memo, create_signal, store,
    untrack,
};
use block_editor_beui::{ContentProjection, Editor, WebViewEvent, WebViewId};

pub(crate) const PAGE: WebViewId = WebViewId(0);

#[derive(Default)]
struct Navigation {
    opened: bool,
    current: Option<String>,
    synchronized: Option<(usize, String)>,
    programmatic: Option<String>,
    natural: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct Session {
    editor: Editor,
    address: ReadSignal<String>,
    set_address: WriteSignal<String>,
    error: ReadSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    inner: Stored<Inner>,
}

struct Inner {
    tab: ContentProjection<BrowserTabContent>,
    navigation: RefCell<Navigation>,
}

impl Session {
    pub(crate) fn new(editor: Editor) -> Self {
        let (address, set_address) = create_signal("about:blank".to_owned());
        let (error, set_error) = create_signal(None::<String>);
        let session = Self {
            editor,
            address,
            set_address,
            error,
            set_error,
            inner: store(Inner {
                tab: editor.block_content::<BrowserTabContent>(),
                navigation: RefCell::default(),
            }),
        };
        let events = editor.web_view_events();
        create_effect(move || {
            events.get();
            untrack(|| session.process_events());
        });
        create_effect(move || session.open());
        create_effect(move || session.synchronize());
        session
    }

    pub(crate) fn address(&self) -> ReadSignal<String> {
        self.address
    }

    pub(crate) fn set_address(&self, address: String) {
        self.set_address.set(address);
    }

    pub(crate) fn error(&self) -> ReadSignal<Option<String>> {
        self.error
    }

    pub(crate) fn back(&self) -> Memo<Option<usize>> {
        let inner = self.inner.get();
        let index = inner.tab.project(|tab| tab.root().index());
        create_memo(move || index.get().checked_sub(1))
    }

    pub(crate) fn forward(&self) -> Memo<Option<usize>> {
        let inner = self.inner.get();
        let step = inner.tab.project(|tab| {
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
        self.editor.host().reload_web_view(PAGE);
    }

    pub(crate) fn focus_app(&self) {
        self.editor.host().focus_app(PAGE);
    }

    pub(crate) fn navigate(&self, address: &str) {
        let url = browser_url(address);
        self.set_address.set(url.clone());
        self.push(url);
    }

    fn open(&self) {
        let inner = self.inner.get();
        if inner.navigation.borrow().opened {
            return;
        }
        let Some((index, url)) = inner.tab.read(|tab| {
            let tab = tab.root();
            (tab.index(), tab.current().url)
        }) else {
            return;
        };
        let mut navigation = inner.navigation.borrow_mut();
        navigation.opened = true;
        navigation.current = Some(url.clone());
        navigation.programmatic = Some(url.clone());
        navigation.synchronized = Some((index, url.clone()));
        drop(navigation);
        self.set_address.set(url.clone());
        self.editor.host().open_web_view(PAGE, url);
    }

    fn process_events(&self) {
        let inner = self.inner.get();
        for event in self.editor.host().take_web_view_events(PAGE) {
            match event {
                WebViewEvent::Navigate(url) => self.navigation_started(url),
                WebViewEvent::Finished(url) => self.navigation_finished(url),
                WebViewEvent::Push(url) => self.push_item(url),
                WebViewEvent::Replace(url) => self.replace(url),
                WebViewEvent::Title(title) => self.title_changed(title),
                WebViewEvent::History(delta) => self.traverse_history(delta as isize),
                WebViewEvent::NewWindow(url) => self.push(url),
                WebViewEvent::Address(url) => inner.navigation.borrow_mut().current = Some(url),
                WebViewEvent::Failed(error) => self.set_error.set(Some(error)),
            }
        }
    }

    fn edit(&self, make: impl FnOnce(&BrowserTab) -> Edit) {
        let inner = self.inner.get();
        let edit = inner.tab.read(|tab| make(&tab.root()));
        if let Some(edit) = edit.filter(|edit| !edit.0.is_empty()) {
            inner.tab.operate(edit);
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
        let inner = self.inner.get();
        let mut navigation = inner.navigation.borrow_mut();
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
        let inner = self.inner.get();
        let mut navigation = inner.navigation.borrow_mut();
        navigation.programmatic = None;
        navigation.natural = None;
        drop(navigation);
        self.set_address.set(url.clone());
        let current = inner.tab.read(|tab| tab.root().current().url);
        if current.as_deref().is_some_and(|current| current != url) {
            self.replace(url);
        }
    }

    fn title_changed(&self, title: String) {
        let inner = self.inner.get();
        let Some(shown) = inner.tab.read(|tab| tab.root().current().url) else {
            return;
        };
        let url = inner.navigation.borrow().current.clone().unwrap_or(shown);
        let item = HistoryItem::new(url, title);
        self.edit(|tab| tab.replace(&item));
    }

    fn item_with_url(&self, url: String) -> HistoryItem {
        let inner = self.inner.get();
        let title = inner
            .tab
            .read(|tab| tab.root().current().title)
            .unwrap_or_default();
        HistoryItem::new(url, title)
    }

    fn traverse_history(&self, delta: isize) {
        let inner = self.inner.get();
        let Some((index, length)) = inner.tab.read(|tab| {
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
        let inner = self.inner.get();
        let Some(selected) = inner.tab.read(|tab| {
            let tab = tab.root();
            (tab.index(), tab.current().url)
        }) else {
            return;
        };
        untrack(|| self.follow(selected));
    }

    fn follow(&self, selected: (usize, String)) {
        let inner = self.inner.get();
        let mut navigation = inner.navigation.borrow_mut();
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
            self.editor.host().load_web_view(PAGE, url);
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
