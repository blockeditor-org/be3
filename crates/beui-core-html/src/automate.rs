use std::fmt::Write as _;
use std::time::Duration;

use beui_types::{Rect, Vec2, pos2};

pub const TIMEOUT: Duration = Duration::from_secs(30);

const ANSWERS: &str = "the HTML core answers tree, ids, find TARGET, wait TEXT, gone TEXT, settle, pause MILLISECONDS, clock stop|run and help; input (click, move, drag, type, key) comes from the browser, as a person's does";

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub depth: usize,
    pub text: String,
    pub test_ids: Vec<String>,
    pub bounds: Option<Rect>,
    pub visible: bool,
}

pub struct Description<'a> {
    pub role: &'a str,
    pub label: Option<&'a str>,
    pub value: Option<&'a str>,
    pub toggled: Option<&'a str>,
    pub disabled: bool,
    pub selected: bool,
    pub focused: bool,
    pub test_ids: &'a [String],
    pub bounds: Option<Rect>,
    pub clip: Rect,
}

pub fn describe(depth: usize, description: Description) -> Line {
    let mut text = description.role.to_owned();
    if let Some(label) = description.label.filter(|label| !label.is_empty()) {
        let _ = write!(text, " {label:?}");
    }
    if let Some(value) = description.value.filter(|value| !value.is_empty()) {
        let _ = write!(text, " value={value:?}");
    }
    if let Some(toggled) = description.toggled {
        let _ = write!(text, " toggled={toggled}");
    }
    if description.disabled {
        text.push_str(" disabled");
    }
    if description.selected {
        text.push_str(" selected");
    }
    if description.focused {
        text.push_str(" focused");
    }
    for test_id in description.test_ids {
        let _ = write!(text, " #{test_id}");
    }
    let mut visible = true;
    if let Some(rect) = description.bounds {
        let _ = write!(text, " {}", place(rect));
        let shown = area(description.clip.intersect(rect));
        if shown <= 0.0 && area(rect) > 0.0 {
            text.push_str(" offscreen");
            visible = false;
        } else if shown < area(rect) {
            text.push_str(" partly offscreen");
        }
    }
    Line {
        depth,
        text,
        test_ids: description.test_ids.to_vec(),
        bounds: description.bounds,
        visible,
    }
}

fn area(rect: Rect) -> f32 {
    rect.width().max(0.0) * rect.height().max(0.0)
}

pub fn place(rect: Rect) -> String {
    format!(
        "at {},{} size {}x{}",
        rect.min.x.round(),
        rect.min.y.round(),
        rect.width().round(),
        rect.height().round()
    )
}

pub fn render(lines: &[Line]) -> String {
    let mut text = String::new();
    for line in lines {
        for _ in 0..line.depth {
            text.push_str("  ");
        }
        text.push_str(&line.text);
        text.push('\n');
    }
    text
}

pub fn ids(lines: &[Line]) -> String {
    let mut named: Vec<(&str, &Line)> = lines
        .iter()
        .flat_map(|line| line.test_ids.iter().map(move |id| (id.as_str(), line)))
        .collect();
    named.sort_by(|a, b| a.0.cmp(b.0));
    named
        .into_iter()
        .map(|(id, line)| {
            let place = line.bounds.map(place).unwrap_or_default();
            let hidden = if line.visible { "" } else { " offscreen" };
            format!("#{id} {place}{hidden}\n")
        })
        .collect()
}

fn numbers(text: &str) -> Option<Vec<f32>> {
    text.split(',')
        .map(|number| number.trim().parse::<f32>().ok())
        .collect()
}

fn placed(line: &Line) -> Result<Rect, String> {
    let rect = line
        .bounds
        .ok_or_else(|| format!("{:?} has no place on screen", line.text))?;
    if !line.visible {
        return Err(format!("{:?} is scrolled out of view", line.text));
    }
    Ok(rect)
}

pub fn locate(target: &str, lines: &[Line]) -> Result<Rect, String> {
    if let Some(id) = target.strip_prefix('#') {
        if let Some((id, offset)) = id.rsplit_once('+')
            && let Some([x, y]) = numbers(offset).as_deref()
        {
            let rect = locate(&format!("#{id}"), lines)?;
            return Ok(Rect::from_min_size(
                rect.min + Vec2::new(*x, *y),
                Vec2::ZERO,
            ));
        }
        let named: Vec<&Line> = lines
            .iter()
            .filter(|line| line.test_ids.iter().any(|named| named == id))
            .collect();
        return match named.as_slice() {
            [] => Err(format!(
                "no node on screen has the test id {id}; `ids` lists them"
            )),
            [line] => placed(line),
            _ => Err(format!(
                "the test id {id} names {} nodes on screen",
                named.len()
            )),
        };
    }
    match numbers(target).as_deref() {
        Some([x, y]) => return Ok(Rect::from_min_size(pos2(*x, *y), Vec2::ZERO)),
        Some([x, y, width, height]) => {
            return Ok(Rect::from_min_size(
                pos2(*x, *y),
                Vec2::new(*width, *height),
            ));
        }
        _ => {}
    }
    let found: Vec<usize> = (0..lines.len())
        .filter(|index| lines[*index].text.contains(target))
        .collect();
    match pick(lines, &found) {
        Some(index) => placed(&lines[index]),
        None if found.is_empty() => Err(format!(
            "no line of the tree contains {target:?}; `tree` prints it"
        )),
        None => {
            let mut message = format!("{} lines of the tree contain {target:?}:\n", found.len());
            for index in &found {
                message.push_str("  ");
                message.push_str(&lines[*index].text);
                message.push('\n');
            }
            message.push_str("name one of them more closely, or use #TEST_ID or X,Y");
            Err(message)
        }
    }
}

fn pick(lines: &[Line], found: &[usize]) -> Option<usize> {
    if let [first, rest @ ..] = found
        && rest.iter().all(|index| {
            lines[*index].text == lines[*first].text && lines[*index].bounds == lines[*first].bounds
        })
    {
        return Some(*first);
    }
    let within = |candidates: &[usize]| {
        let first = *candidates.first()?;
        let depth = lines[first].depth;
        let end = lines[first + 1..]
            .iter()
            .position(|line| line.depth <= depth)
            .map_or(lines.len(), |offset| first + 1 + offset);
        candidates.iter().all(|index| *index < end).then_some(first)
    };
    within(found).or_else(|| {
        let shown: Vec<usize> = found
            .iter()
            .copied()
            .filter(|index| lines[*index].visible)
            .collect();
        within(&shown)
    })
}

pub enum Command {
    Tree,
    Ids,
    Find(String),
    Wait(String),
    Gone(String),
    Settle,
    Pause(Duration),
    Clock(bool),
    Help,
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
    let joined = || arguments.join(" ");
    let named = |what: &str| -> Result<String, String> {
        match arguments.is_empty() {
            true => Err(format!("{command} needs {what}")),
            false => Ok(joined()),
        }
    };
    let command = match command.as_str() {
        "tree" => Command::Tree,
        "ids" => Command::Ids,
        "find" => Command::Find(named("a TARGET")?),
        "wait" => Command::Wait(named("TEXT")?),
        "gone" => Command::Gone(named("TEXT")?),
        "settle" => Command::Settle,
        "help" => Command::Help,
        "pause" => {
            let milliseconds: u64 = named("MILLISECONDS")?
                .parse()
                .map_err(|_| format!("{} is not a number of milliseconds", joined()))?;
            Command::Pause(Duration::from_millis(milliseconds))
        }
        "clock" => match joined().as_str() {
            "stop" => Command::Clock(false),
            "run" => Command::Clock(true),
            other => return Err(format!("clock takes stop or run, not {other:?}")),
        },
        other => return Err(format!("{other} is not a command here: {ANSWERS}")),
    };
    Ok(Request { command, timeout })
}

#[cfg(target_arch = "wasm32")]
pub use web::{automate, lines};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::collections::HashMap;
    use std::time::Duration;

    use beui_types::{Rect, pos2, vec2};
    use wasm_bindgen::JsCast;

    use super::{Command, Description, Line, describe, ids, locate, parse, place, render};
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
            let Some(about) = about(dom, &self.abouts, &self.keys) else {
                self.children(dom, depth, clip);
                return;
            };
            let Some(element) = dom.dyn_ref::<web_sys::Element>() else {
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
            let line = match (&about.accessibility, &text) {
                (Some(node), _) => {
                    let role = format!("{:?}", node.role());
                    let numeric = node
                        .numeric_value()
                        .map(|value| ((value * 1000.0).round() / 1000.0).to_string());
                    let value = node.value().map(str::to_owned).or(numeric);
                    let toggled = node.toggled().map(|toggled| format!("{toggled:?}"));
                    Some(describe(
                        depth,
                        Description {
                            role: &role,
                            label: node.label(),
                            value: value.as_deref(),
                            toggled: toggled.as_deref(),
                            disabled: node.is_disabled(),
                            selected: node.is_selected() == Some(true),
                            focused,
                            test_ids: &about.test_ids,
                            bounds: Some(rect),
                            clip,
                        },
                    ))
                }
                (None, Some(text)) => Some(describe(
                    depth,
                    Description {
                        role: "Label",
                        label: None,
                        value: Some(text),
                        toggled: None,
                        disabled: false,
                        selected: false,
                        focused,
                        test_ids: &about.test_ids,
                        bounds: Some(rect),
                        clip,
                    },
                )),
                (None, None) if !about.test_ids.is_empty() => Some(describe(
                    depth,
                    Description {
                        role: "GenericContainer",
                        label: None,
                        value: None,
                        toggled: None,
                        disabled: false,
                        selected: false,
                        focused,
                        test_ids: &about.test_ids,
                        bounds: Some(rect),
                        clip,
                    },
                )),
                (None, None) => None,
            };
            let shown = line.is_some();
            self.lines.extend(line);
            if about.kinds.contains(&"Text") {
                return;
            }
            let clip = match about.kinds.contains(&"Offset") {
                true => clip.intersect(rect),
                false => clip,
            };
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
        let mut by_index = HashMap::new();
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
                test_ids: &[],
                bounds: root_rect,
                clip: clip_everywhere(),
            },
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
            if !crate::frame::pending() {
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

    async fn wait(text: &str, shown: bool, timeout: Duration) -> Result<String, String> {
        let deadline = wall_milliseconds() + timeout.as_secs_f64() * 1000.0;
        loop {
            let lines = lines();
            let found = lines.iter().find(|line| line.text.contains(text));
            match (found, shown) {
                (Some(line), true) => return Ok(format!("{}\n", line.text)),
                (None, false) => return Ok(String::new()),
                _ => {}
            }
            if wall_milliseconds() > deadline {
                let what = match shown {
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
            Command::Wait(text) => wait(&text, true, timeout).await,
            Command::Gone(text) => wait(&text, false, timeout).await,
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
