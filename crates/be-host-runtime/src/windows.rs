use std::cell::RefCell;

use be_sdk_api::WindowRequest;

thread_local! {
    static PENDING: RefCell<Vec<(String, WindowRequest)>> = const { RefCell::new(Vec::new()) };
    static FORGOTTEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn request_windows(app: &str, requests: Vec<WindowRequest>) {
    if requests.is_empty() {
        return;
    }
    PENDING.with(|pending| {
        pending.borrow_mut().extend(
            requests
                .into_iter()
                .map(|request| (app.to_owned(), request)),
        )
    });
    be_host_base::host::request_repaint();
}

pub fn forget_windows(app: &str) {
    FORGOTTEN.with(|forgotten| forgotten.borrow_mut().push(app.to_owned()));
    be_host_base::host::request_repaint();
}

pub fn take_window_requests() -> (Vec<(String, WindowRequest)>, Vec<String>) {
    (
        PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut())),
        FORGOTTEN.with(|forgotten| std::mem::take(&mut *forgotten.borrow_mut())),
    )
}
