use std::time::Duration;

use beui_types::drive::Command;

pub const TIMEOUT: Duration = Duration::from_secs(30);

const ANSWERS: &str = "the HTML core answers tree, ids, find TARGET, reveal TARGET (scrolls a target a scroll cuts into view and says where it shows, which is where web-test.mjs puts input, ending in revealed when it scrolled), shown TARGET (says where a target shows, without scrolling), wait TEXT [over TARGET], gone TEXT [over TARGET], settle, pause MILLISECONDS, clock stop|run and help; input (click, move, drag, type, key) comes from the browser, as a person's does";

pub type Line = beui_types::drive::Line<Option<crate::node::NodeId>>;

pub fn hidden(target: &str, lines: &[Line]) -> Option<crate::node::NodeId> {
    beui_types::target::hidden(target, lines).and_then(|line| line.node)
}

pub struct Request {
    pub command: Command,
    pub timeout: Duration,
}

pub fn parse(words: &[String]) -> Result<Request, String> {
    let mut timeout = TIMEOUT;
    let mut rest = words;
    while let Some((first, others)) = rest.split_first()
        && first.starts_with("--")
    {
        if let Some(seconds) = first.strip_prefix("--timeout=") {
            let seconds: f64 = seconds
                .parse()
                .map_err(|_| format!("{seconds} is not a number of seconds"))?;
            timeout = Duration::from_secs_f64(seconds.max(0.0));
        } else if first.starts_with("--into=") {
            return Err("the HTML core has no layers to send input into".to_owned());
        }
        rest = others;
    }
    let Some((command, arguments)) = rest.split_first() else {
        return Err(format!("no command; {ANSWERS}"));
    };
    match beui_types::drive::parse(command, arguments) {
        Some(command) => Ok(Request {
            command: command?,
            timeout,
        }),
        None => Err(format!("{command} is not a command here: {ANSWERS}")),
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::{automate, lines};

#[cfg(target_arch = "wasm32")]
mod web {
    use stable_hash::HashMap;
    use std::time::Duration;

    use beui_types::{Rect, pos2, vec2};
    use wasm_bindgen::JsCast;

    use beui_types::drive::{Description, describe, ids, render, waited};
    use beui_types::target::{locate, place, shown_at};

    use super::{Command, Line, hidden, parse};
    use crate::document::{About, about_elements};

    fn clip_everywhere() -> Rect {
        Rect::from_min_size(pos2(-1.0e9, -1.0e9), vec2(2.0e9, 2.0e9))
    }

    fn rect_of(element: &web_sys::Element, ratio: f32) -> Rect {
        let bounds = element.get_bounding_client_rect();
        Rect::from_min_size(
            pos2(bounds.left() as f32 * ratio, bounds.top() as f32 * ratio),
            vec2(
                bounds.width() as f32 * ratio,
                bounds.height() as f32 * ratio,
            ),
        )
    }

    fn text_of(dom: &web_sys::Node, abouts: &HashMap<u32, &About>, keys: &js_sys::Map) -> String {
        let mut text = String::new();
        let mut child = dom.first_child();
        while let Some(node) = child {
            match node.node_type() {
                web_sys::Node::TEXT_NODE => text.push_str(&node.text_content().unwrap_or_default()),
                web_sys::Node::ELEMENT_NODE => {
                    let icon = about(&node, abouts, keys)
                        .is_some_and(|about| about.kinds.contains(&"Icon"));
                    if !icon {
                        text.push_str(&text_of(&node, abouts, keys));
                    }
                }
                _ => {}
            }
            child = node.next_sibling();
        }
        text
    }

    fn about<'a>(
        dom: &web_sys::Node,
        abouts: &HashMap<u32, &'a About>,
        keys: &js_sys::Map,
    ) -> Option<&'a About> {
        let key = keys.get(dom).as_f64()?;
        abouts.get(&(key as u32)).copied()
    }

    struct Walk<'a> {
        abouts: HashMap<u32, &'a About>,
        keys: js_sys::Map,
        focused: Option<web_sys::Node>,
        ratio: f32,
        viewport: Rect,
        lines: Vec<Line>,
    }

    impl Walk<'_> {
        fn within(&self, element: &web_sys::Element, clip: Rect) -> Rect {
            match crate::dom::clips(element) {
                true => clip.intersect(rect_of(element, self.ratio)),
                false => clip,
            }
        }

        fn children(&mut self, dom: &web_sys::Node, depth: usize, clip: Rect) {
            let mut child = dom.first_child();
            while let Some(node) = child {
                if node.node_type() == web_sys::Node::ELEMENT_NODE {
                    self.element(&node, depth, clip);
                }
                child = node.next_sibling();
            }
        }

        fn element(&mut self, dom: &web_sys::Node, depth: usize, clip: Rect) {
            let Some(element) = dom.dyn_ref::<web_sys::Element>() else {
                return;
            };
            let Some(about) = about(dom, &self.abouts, &self.keys) else {
                self.children(dom, depth, self.within(element, clip));
                return;
            };
            let rendered = crate::dom::call(element.as_ref(), "checkVisibility", &[])
                .and_then(|shown| shown.as_bool())
                .unwrap_or(true)
                || crate::dom::window()
                    .and_then(|window| {
                        crate::dom::call(window.as_ref(), "getComputedStyle", &[element.into()])
                    })
                    .and_then(|style| js_sys::Reflect::get(&style, &"display".into()).ok())
                    .and_then(|display| display.as_string())
                    .is_some_and(|display| display == "contents");
            if !rendered {
                return;
            }
            let clip = match about.kinds.contains(&"OverlayLayer") {
                true => self.viewport,
                false => clip,
            };
            let rect = rect_of(element, self.ratio);
            let focused = self.focused.as_ref() == Some(dom);
            let text = about
                .kinds
                .contains(&"Text")
                .then(|| text_of(dom, &self.abouts, &self.keys))
                .filter(|text| !text.is_empty());
            let described = match (&about.accessibility, &text) {
                (Some(node), text) => {
                    let numeric = node
                        .numeric_value()
                        .map(|value| ((value * 1000.0).round() / 1000.0).to_string());
                    let value = node.value().map(str::to_owned).or(numeric);
                    let unnamed = node.label().is_none() && value.is_none();
                    let (label, value) = match (unnamed, text) {
                        (true, Some(text)) if node.role() == accesskit::Role::Label => {
                            (None, Some(text.clone()))
                        }
                        (true, Some(text)) => (Some(text.as_str()), None),
                        _ => (node.label(), value),
                    };
                    Some((format!("{:?}", node.role()), label, value, Some(node)))
                }
                (None, Some(text)) => Some(("Label".to_owned(), None, Some(text.clone()), None)),
                (None, None) if !about.test_ids.is_empty() => {
                    Some(("GenericContainer".to_owned(), None, None, None))
                }
                (None, None) => None,
            };
            let line = described.map(|(role, label, value, node)| {
                let toggled = node
                    .and_then(accesskit::Node::toggled)
                    .map(|toggled| format!("{toggled:?}"));
                describe(
                    depth,
                    Description {
                        role: &role,
                        label,
                        value: value.as_deref(),
                        toggled: toggled.as_deref(),
                        disabled: node.is_some_and(accesskit::Node::is_disabled),
                        selected: node.is_some_and(|node| node.is_selected() == Some(true)),
                        focused,
                        test_ids: about.test_ids.clone(),
                        bounds: Some(rect),
                        clip,
                    },
                    Some(about.node),
                )
            });
            let shown = line.is_some();
            self.lines.extend(line);
            if about.kinds.contains(&"Text") {
                return;
            }
            let clip = self.within(element, clip);
            self.children(dom, depth + usize::from(shown), clip);
        }
    }

    pub fn lines() -> Vec<Line> {
        let Some(root) = crate::run::root() else {
            return Vec::new();
        };
        let Some(root_dom) = crate::dom::dom(root) else {
            return Vec::new();
        };
        let window = crate::dom::window();
        let ratio = window
            .as_ref()
            .map_or(1.0, |window| window.device_pixel_ratio() as f32);
        let viewport = window.as_ref().map_or_else(clip_everywhere, |window| {
            let size = |value: Result<wasm_bindgen::JsValue, _>| {
                value.ok().and_then(|value| value.as_f64()).unwrap_or(0.0) as f32 * ratio
            };
            Rect::from_min_size(
                pos2(0.0, 0.0),
                vec2(size(window.inner_width()), size(window.inner_height())),
            )
        });
        let abouts = about_elements();
        let keys = js_sys::Map::new();
        let mut by_index = HashMap::default();
        for (index, about) in abouts.iter().enumerate() {
            if let Some(dom) = about.element.dom() {
                keys.set(&dom, &(index as f64).into());
                by_index.insert(index as u32, about);
            }
        }
        let focused = crate::focus::focused_node().and_then(crate::dom::dom);
        let title = crate::dom::document()
            .map(|document| document.title())
            .unwrap_or_default();
        let mut walk = Walk {
            abouts: by_index,
            keys,
            focused,
            ratio,
            viewport,
            lines: Vec::new(),
        };
        let root_rect = root_dom
            .dyn_ref::<web_sys::Element>()
            .map(|element| rect_of(element, ratio));
        walk.lines.push(describe(
            0,
            Description {
                role: "Window",
                label: Some(&title),
                value: None,
                toggled: None,
                disabled: false,
                selected: false,
                focused: false,
                test_ids: Vec::new(),
                bounds: root_rect,
                clip: clip_everywhere(),
            },
            None,
        ));
        walk.children(&root_dom, 1, viewport);
        walk.lines
    }

    async fn next_frame() {
        let Some(window) = crate::dom::window() else {
            return;
        };
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            let _ = window.request_animation_frame(&resolve);
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }

    async fn after(delay: Duration) {
        let Some(window) = crate::dom::window() else {
            return;
        };
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                &resolve,
                i32::try_from(delay.as_millis()).unwrap_or(i32::MAX),
            );
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }

    fn wall_milliseconds() -> f64 {
        crate::dom::window()
            .and_then(|window| window.performance())
            .map_or(0.0, |performance| performance.now())
    }

    async fn settle(timeout: Duration) -> Result<(), String> {
        let deadline = wall_milliseconds() + timeout.as_secs_f64() * 1000.0;
        loop {
            next_frame().await;
            if !crate::frame::busy() {
                return Ok(());
            }
            if wall_milliseconds() > deadline {
                return Err(format!(
                    "the page did not settle in {}s: it kept asking for frames",
                    timeout.as_secs_f32()
                ));
            }
        }
    }

    async fn wait(
        text: &str,
        over: Option<&str>,
        present: bool,
        timeout: Duration,
    ) -> Result<String, String> {
        let deadline = wall_milliseconds() + timeout.as_secs_f64() * 1000.0;
        let over = match over {
            Some(at) => Some(locate(at, &lines())?.center()),
            None => None,
        };
        loop {
            let lines = lines();
            match (waited(&lines, text, over), present) {
                (Some(line), true) => return Ok(format!("{}\n", line.text)),
                (None, false) => return Ok(String::new()),
                _ => {}
            }
            if wall_milliseconds() > deadline {
                let what = match present {
                    true => "no line of the tree contains",
                    false => "a line of the tree still contains",
                };
                return Err(format!(
                    "after {}s {what} {text:?}; the tree:\n{}",
                    timeout.as_secs_f32(),
                    render(&lines)
                ));
            }
            next_frame().await;
        }
    }

    pub async fn automate(words: Vec<String>) -> Result<String, String> {
        let request = parse(&words)?;
        let timeout = request.timeout;
        match request.command {
            Command::Tree => Ok(render(&lines())),
            Command::Ids => Ok(ids(&lines())),
            Command::Find(target) => Ok(format!("{}\n", place(locate(&target, &lines())?))),
            Command::Reveal(target) => {
                let Some(node) = hidden(&target, &lines()) else {
                    return Ok(format!("{}\n", place(shown_at(&target, &lines())?)));
                };
                crate::scroll::reveal(node);
                settle(timeout).await?;
                Ok(format!(
                    "{} revealed\n",
                    place(shown_at(&target, &lines())?)
                ))
            }
            Command::Shown(target) => Ok(format!("{}\n", place(shown_at(&target, &lines())?))),
            Command::Wait {
                text,
                over,
                present,
            } => wait(&text, over.as_deref(), present, timeout).await,
            Command::Settle => settle(timeout).await.map(|()| String::new()),
            Command::Pause(by) => {
                if crate::frame::frozen() {
                    crate::frame::advance_clock(by);
                } else {
                    after(by).await;
                }
                settle(timeout).await.map(|()| String::new())
            }
            Command::Clock(running) => {
                crate::frame::freeze_clock(!running);
                Ok(String::new())
            }
            Command::Help => Ok(format!("{}.\n", super::ANSWERS)),
        }
    }
}
