#[cfg(not(target_os = "android"))]
pub use desktop::Accessibility;

#[cfg(not(target_os = "android"))]
mod desktop {
    use accesskit::TreeUpdate;
    use accesskit_winit::{Adapter, Event};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    pub struct Accessibility(Adapter);

    impl Accessibility {
        pub fn new<T: From<Event> + Send + 'static>(
            event_loop: &ActiveEventLoop,
            window: &Window,
            proxy: EventLoopProxy<T>,
        ) -> Self {
            Self(Adapter::with_event_loop_proxy(event_loop, window, proxy))
        }

        pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
            self.0.update_if_active(updater);
        }

        pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
            self.0.process_event(window, event);
        }
    }
}

#[cfg(target_os = "android")]
pub use android::Accessibility;

#[cfg(target_os = "android")]
mod android {
    use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
    use accesskit_android::InjectingAdapter;
    use accesskit_android::jni::JavaVM;
    use accesskit_android::jni::objects::{JObject, JValue};
    use accesskit_winit::{Event, WindowEvent as AccessKitWindowEvent};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::platform::android::ActiveEventLoopExtAndroid;
    use winit::window::{Window, WindowId};

    const ANDROID_R_ID_CONTENT: i32 = 0x0102_0002;

    pub struct Accessibility(Option<InjectingAdapter>);

    impl Accessibility {
        pub fn new<T: From<Event> + Send + 'static>(
            event_loop: &ActiveEventLoop,
            window: &Window,
            proxy: EventLoopProxy<T>,
        ) -> Self {
            match inject(event_loop, window.id(), proxy) {
                Ok(adapter) => Self(Some(adapter)),
                Err(error) => {
                    eprintln!("accessibility is unavailable: {error}");
                    Self(None)
                }
            }
        }

        pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
            if let Some(adapter) = &mut self.0 {
                adapter.update_if_active(updater);
            }
        }

        pub fn process_event(&mut self, _window: &Window, _event: &WindowEvent) {}
    }

    fn inject<T: From<Event> + Send + 'static>(
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        proxy: EventLoopProxy<T>,
    ) -> Result<InjectingAdapter, String> {
        let app = event_loop.android_app();
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }
            .map_err(|error| error.to_string())?;
        let mut env = vm
            .attach_current_thread_permanently()
            .map_err(|error| error.to_string())?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
        let found = env
            .call_method(
                &activity,
                "findViewById",
                "(I)Landroid/view/View;",
                &[JValue::Int(ANDROID_R_ID_CONTENT)],
            )
            .and_then(|content| content.l())
            .and_then(|content| {
                if content.is_null() {
                    return Ok(content);
                }
                env.call_method(
                    &content,
                    "getChildAt",
                    "(I)Landroid/view/View;",
                    &[JValue::Int(0)],
                )?
                .l()
            });
        let view = match found {
            Ok(view) if !view.is_null() => view,
            Ok(_) => return Err("the activity has no content view".to_owned()),
            Err(error) => {
                let _ = env.exception_clear();
                return Err(error.to_string());
            }
        };
        let activation_handler = Handler {
            window_id,
            proxy: proxy.clone(),
        };
        let action_handler = Handler { window_id, proxy };
        Ok(InjectingAdapter::new(
            &mut env,
            &view,
            activation_handler,
            action_handler,
        ))
    }

    struct Handler<T: From<Event> + Send + 'static> {
        window_id: WindowId,
        proxy: EventLoopProxy<T>,
    }

    impl<T: From<Event> + Send + 'static> Handler<T> {
        fn send(&self, window_event: AccessKitWindowEvent) {
            let event = Event {
                window_id: self.window_id,
                window_event,
            };
            let _ = self.proxy.send_event(event.into());
        }
    }

    impl<T: From<Event> + Send + 'static> ActivationHandler for Handler<T> {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            self.send(AccessKitWindowEvent::InitialTreeRequested);
            None
        }
    }

    impl<T: From<Event> + Send + 'static> ActionHandler for Handler<T> {
        fn do_action(&mut self, request: ActionRequest) {
            self.send(AccessKitWindowEvent::ActionRequested(request));
        }
    }
}
