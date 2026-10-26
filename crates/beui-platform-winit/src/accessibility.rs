use std::sync::mpsc::{Receiver, Sender, channel};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use winit::event::WindowEvent;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

pub(crate) enum Request {
    InitialTree,
    Action(ActionRequest),
    Deactivated,
}

struct Handler {
    requests: Sender<Request>,
    proxy: EventLoopProxy,
}

impl Handler {
    fn send(&self, request: Request) {
        if self.requests.send(request).is_ok() {
            self.proxy.wake_up();
        }
    }
}

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.send(Request::InitialTree);
        None
    }
}

impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        self.send(Request::Action(request));
    }
}

impl DeactivationHandler for Handler {
    fn deactivate_accessibility(&mut self) {
        self.send(Request::Deactivated);
    }
}

pub(crate) struct Accessibility {
    platform: platform::Adapter,
    requests: Receiver<Request>,
}

impl Accessibility {
    pub(crate) fn new(window: &dyn Window, proxy: EventLoopProxy) -> Self {
        let (requests, received) = channel();
        let handler = || Handler {
            requests: requests.clone(),
            proxy: proxy.clone(),
        };
        Self {
            platform: platform::Adapter::new(window, handler(), handler(), handler()),
            requests: received,
        }
    }

    pub(crate) fn take_requests(&self) -> Vec<Request> {
        self.requests.try_iter().collect()
    }

    pub(crate) fn window_event(&mut self, window: &dyn Window, event: &WindowEvent) {
        self.platform.window_event(window, event);
    }

    pub(crate) fn update_if_active(&mut self, tree: impl FnOnce() -> TreeUpdate) {
        self.platform.update_if_active(tree);
    }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
mod platform {
    use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, Rect, TreeUpdate};
    use winit::event::WindowEvent;
    use winit::window::Window;

    pub(super) struct Adapter(accesskit_unix::Adapter);

    impl Adapter {
        pub(super) fn new(
            _window: &dyn Window,
            activation: impl ActivationHandler + Send + 'static,
            action: impl ActionHandler + Send + 'static,
            deactivation: impl DeactivationHandler + Send + 'static,
        ) -> Self {
            Self(accesskit_unix::Adapter::new(
                activation,
                action,
                deactivation,
            ))
        }

        pub(super) fn window_event(&mut self, window: &dyn Window, event: &WindowEvent) {
            match event {
                WindowEvent::Moved(_) | WindowEvent::SurfaceResized(_) => {
                    let outer = window.outer_position().unwrap_or_default();
                    let inner = window.surface_position();
                    let outer_size = window.outer_size();
                    let size = window.surface_size();
                    let rect = |x: i32, y: i32, width: u32, height: u32| {
                        Rect::from_origin_size(
                            (f64::from(x), f64::from(y)),
                            (f64::from(width), f64::from(height)),
                        )
                    };
                    self.0.set_root_window_bounds(
                        rect(outer.x, outer.y, outer_size.width, outer_size.height),
                        rect(
                            outer.x + inner.x,
                            outer.y + inner.y,
                            size.width,
                            size.height,
                        ),
                    );
                }
                WindowEvent::Focused(focused) => self.0.update_window_focus_state(*focused),
                _ => {}
            }
        }

        pub(super) fn update_if_active(&mut self, tree: impl FnOnce() -> TreeUpdate) {
            self.0.update_if_active(tree);
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
    use accesskit_windows::{HWND, SubclassingAdapter};
    use winit::event::WindowEvent;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    pub(super) struct Adapter(Option<SubclassingAdapter>);

    impl Adapter {
        pub(super) fn new(
            window: &dyn Window,
            activation: impl ActivationHandler + 'static,
            action: impl ActionHandler + Send + 'static,
            _deactivation: impl DeactivationHandler + 'static,
        ) -> Self {
            let hwnd = match window.window_handle().map(|handle| handle.as_raw()) {
                Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as *mut _,
                _ => return Self(None),
            };
            Self(Some(SubclassingAdapter::new(
                HWND(hwnd),
                activation,
                action,
            )))
        }

        pub(super) fn window_event(&mut self, _window: &dyn Window, _event: &WindowEvent) {}

        pub(super) fn update_if_active(&mut self, tree: impl FnOnce() -> TreeUpdate) {
            if let Some(adapter) = &mut self.0
                && let Some(events) = adapter.update_if_active(tree)
            {
                events.raise();
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
    use accesskit_macos::SubclassingAdapter;
    use winit::event::WindowEvent;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    pub(super) struct Adapter(Option<SubclassingAdapter>);

    impl Adapter {
        pub(super) fn new(
            window: &dyn Window,
            activation: impl ActivationHandler + 'static,
            action: impl ActionHandler + 'static,
            _deactivation: impl DeactivationHandler + 'static,
        ) -> Self {
            let view = match window.window_handle().map(|handle| handle.as_raw()) {
                Ok(RawWindowHandle::AppKit(handle)) => handle.ns_view.as_ptr(),
                _ => return Self(None),
            };
            // unsafe(ffi)
            Self(Some(unsafe {
                SubclassingAdapter::new(view, activation, action)
            }))
        }

        pub(super) fn window_event(&mut self, _window: &dyn Window, event: &WindowEvent) {
            if let (Some(adapter), WindowEvent::Focused(focused)) = (&mut self.0, event)
                && let Some(events) = adapter.update_view_focus_state(*focused)
            {
                events.raise();
            }
        }

        pub(super) fn update_if_active(&mut self, tree: impl FnOnce() -> TreeUpdate) {
            if let Some(adapter) = &mut self.0
                && let Some(events) = adapter.update_if_active(tree)
            {
                events.raise();
            }
        }
    }
}

#[cfg(any(target_os = "ios", not(any(unix, target_os = "windows"))))]
mod platform {
    use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
    use winit::event::WindowEvent;
    use winit::window::Window;

    pub(super) struct Adapter;

    impl Adapter {
        pub(super) fn new(
            _window: &dyn Window,
            _activation: impl ActivationHandler + 'static,
            _action: impl ActionHandler + 'static,
            _deactivation: impl DeactivationHandler + 'static,
        ) -> Self {
            Self
        }

        pub(super) fn window_event(&mut self, _window: &dyn Window, _event: &WindowEvent) {}

        pub(super) fn update_if_active(&mut self, _tree: impl FnOnce() -> TreeUpdate) {}
    }
}
