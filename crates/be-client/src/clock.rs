use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll, Waker},
    time::Duration,
};

#[derive(Clone, Default)]
pub struct Clock(Kind);

#[derive(Clone, Default)]
enum Kind {
    #[default]
    Real,
    Manual(Arc<Mutex<Hand>>),
}

#[derive(Default)]
struct Hand {
    now: Duration,
    sleepers: Vec<Waker>,
}

impl Clock {
    pub fn real() -> Self {
        Self(Kind::Real)
    }

    pub fn manual() -> Self {
        Self(Kind::Manual(Arc::default()))
    }

    pub fn now(&self) -> Duration {
        match &self.0 {
            Kind::Real => real::now(),
            Kind::Manual(hand) => hand.lock().unwrap_or_else(PoisonError::into_inner).now,
        }
    }

    pub fn at(&self, instant: std::time::Instant) -> Duration {
        match &self.0 {
            Kind::Real => {
                let ago = std::time::Instant::now().saturating_duration_since(instant);
                real::now().saturating_sub(ago)
            }
            Kind::Manual(_) => self.now(),
        }
    }

    pub fn advance(&self, by: Duration) {
        let Kind::Manual(hand) = &self.0 else {
            panic!("only a manual clock can be moved on");
        };
        let sleepers = {
            let mut hand = hand.lock().unwrap_or_else(PoisonError::into_inner);
            hand.now += by;
            std::mem::take(&mut hand.sleepers)
        };
        for sleeper in sleepers {
            sleeper.wake();
        }
    }

    pub fn sleep(&self, duration: Duration) -> Sleep {
        self.sleep_until(self.now() + duration)
    }

    pub fn sleep_until(&self, at: Duration) -> Sleep {
        match &self.0 {
            Kind::Real => Sleep(Wait::Real(real::sleep_until(at))),
            Kind::Manual(hand) => Sleep(Wait::Manual {
                hand: Arc::clone(hand),
                at,
            }),
        }
    }
}

pub struct Sleep(Wait);

enum Wait {
    Real(real::Sleep),
    Manual {
        hand: Arc<Mutex<Hand>>,
        at: Duration,
    },
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        match &mut self.get_mut().0 {
            Wait::Real(sleep) => Pin::new(sleep).poll(context),
            Wait::Manual { hand, at } => {
                let mut hand = hand.lock().unwrap_or_else(PoisonError::into_inner);
                if hand.now >= *at {
                    return Poll::Ready(());
                }
                if !hand
                    .sleepers
                    .iter()
                    .any(|sleeper| sleeper.will_wake(context.waker()))
                {
                    hand.sleepers.push(context.waker().clone());
                }
                Poll::Pending
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod real {
    use std::{sync::OnceLock, time::Duration};

    pub(super) type Sleep = std::pin::Pin<Box<tokio::time::Sleep>>;

    fn origin() -> std::time::Instant {
        static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
        *ORIGIN.get_or_init(std::time::Instant::now)
    }

    pub(super) fn now() -> Duration {
        origin().elapsed()
    }

    pub(super) fn sleep_until(at: Duration) -> Sleep {
        Box::pin(tokio::time::sleep_until(tokio::time::Instant::from_std(
            origin() + at,
        )))
    }
}

#[cfg(target_arch = "wasm32")]
mod real {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };

    use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
    use wasm_bindgen_futures::JsFuture;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = setTimeout)]
        fn set_timeout(handler: &js_sys::Function, milliseconds: f64) -> JsValue;
        #[wasm_bindgen(js_name = clearTimeout)]
        fn clear_timeout(id: &JsValue);
        #[wasm_bindgen(js_namespace = performance, js_name = now)]
        fn performance_now() -> f64;
    }

    fn time_origin() -> f64 {
        thread_local! {
            static ORIGIN: f64 = js_sys::Reflect::get(&js_sys::global(), &"performance".into())
                .and_then(|performance| js_sys::Reflect::get(&performance, &"timeOrigin".into()))
                .ok()
                .and_then(|origin| origin.as_f64())
                .unwrap_or_default();
        }
        ORIGIN.with(|origin| *origin)
    }

    pub(super) struct Sleep {
        id: JsValue,
        fired: JsFuture,
    }

    pub(super) fn now() -> Duration {
        Duration::from_secs_f64((time_origin() + performance_now()).max(0.0) / 1000.0)
    }

    pub(super) fn sleep_until(at: Duration) -> Sleep {
        let wait = at.saturating_sub(now()).as_secs_f64() * 1000.0;
        let mut id = JsValue::UNDEFINED;
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            id = set_timeout(&resolve, wait);
        });
        Sleep {
            id,
            fired: JsFuture::from(promise),
        }
    }

    impl Future for Sleep {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
            Pin::new(&mut self.fired).poll(context).map(|_| ())
        }
    }

    impl Drop for Sleep {
        fn drop(&mut self) {
            clear_timeout(&self.id);
        }
    }
}
