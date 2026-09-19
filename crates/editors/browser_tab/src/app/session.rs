use std::cell::RefCell;
use std::rc::Rc;

use block_client::blocks::web_browser_tab::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};
use block_editor_plugin::beui::reactive::{
    Memo, ReadSignal, WriteSignal, create_memo, create_signal,
};
use block_editor_plugin::{BlockProjection, Editor, WebViewEvent};

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
    tab: Rc<BlockProjection<WebBrowserTab>>,
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
            tab: editor.block::<WebBrowserTab>(),
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
        let index = self.tab.project(|tab| tab.index());
        create_memo(move || index.get().checked_sub(1))
    }

    pub(crate) fn forward(&self) -> Memo<Option<usize>> {
        let step = self.tab.project(|tab| (tab.index(), tab.can_go_forward()));
        create_memo(move || {
            let (index, can) = step.get();
            can.then_some(index + 1)
        })
    }

    pub(crate) fn go(&self, index: usize) {
        self.tab.operate(WebBrowserTabOperation::History(index));
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
        let Some(url) = self
            .tab
            .handle()
            .read()
            .map(|tab| tab.current().url.clone())
        else {
            return;
        };
        let index = self.tab.handle().read().map(|tab| tab.index()).unwrap_or(0);
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

    fn push_item(&self, url: String) {
        self.tab
            .operate(WebBrowserTabOperation::Push(self.item_with_url(url)));
    }

    fn push(&self, url: String) {
        self.tab.operate(WebBrowserTabOperation::Push(HistoryItem {
            url,
            title: String::new(),
        }));
    }

    fn replace(&self, url: String) {
        self.tab
            .operate(WebBrowserTabOperation::Replace(self.item_with_url(url)));
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
        let current = self
            .tab
            .handle()
            .read()
            .map(|tab| tab.current().url.clone());
        if current.as_deref().is_some_and(|current| current != url) {
            self.replace(url);
        }
    }

    fn title_changed(&self, title: String) {
        let Some(tab) = self.tab.handle().read() else {
            return;
        };
        let url = self
            .navigation
            .borrow()
            .current
            .clone()
            .unwrap_or_else(|| tab.current().url.clone());
        drop(tab);
        self.tab
            .operate(WebBrowserTabOperation::Replace(HistoryItem { url, title }));
    }

    fn item_with_url(&self, url: String) -> HistoryItem {
        let title = self
            .tab
            .handle()
            .read()
            .map(|tab| tab.current().title.clone())
            .unwrap_or_default();
        HistoryItem { url, title }
    }

    fn traverse_history(&self, delta: isize) {
        let Some(tab) = self.tab.handle().read() else {
            return;
        };
        let Some(index) = tab.index().checked_add_signed(delta) else {
            return;
        };
        if index < tab.history().len() {
            drop(tab);
            self.go(index);
        }
    }

    fn synchronize(&self) {
        let Some(tab) = self.tab.handle().read() else {
            return;
        };
        let selected = (tab.index(), tab.current().url.clone());
        drop(tab);
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
