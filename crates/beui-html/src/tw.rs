use crate::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Style(Rc<StyleData>);

struct StyleData {
    declaration: &'static str,
    selector: &'static str,
    media: &'static str,
    children: RefCell<ChildList<Style>>,
}

impl ChildValue for Style {}
beui_tree::child_type!(Style);
impl SlotChild for Style {
    type Stored = Self;
    fn store(self) -> Self {
        self
    }
}

struct StyleHost(Weak<StyleData>);
impl SlotHost<Style> for StyleHost {
    fn append(&self, style: Style) {
        if let Some(data) = self.0.upgrade() {
            data.children.borrow_mut().push(style);
        }
    }
    fn open(&self) -> SlotId {
        self.0
            .upgrade()
            .expect("style alive during mount")
            .children
            .borrow_mut()
            .open()
    }
    fn fill(&self, slot: SlotId, styles: Vec<Style>) {
        if let Some(data) = self.0.upgrade() {
            data.children.borrow_mut().fill(slot, styles);
        }
    }
}

struct ElementHost(Weak<NodeData>);
impl SlotHost<Style> for ElementHost {
    fn append(&self, style: Style) {
        if let Some(data) = self.0.upgrade() {
            data.css.borrow_mut().push(style);
        }
    }
    fn open(&self) -> SlotId {
        self.0
            .upgrade()
            .expect("element alive during mount")
            .css
            .borrow_mut()
            .open()
    }
    fn fill(&self, slot: SlotId, styles: Vec<Style>) {
        if let Some(data) = self.0.upgrade() {
            data.css.borrow_mut().fill(slot, styles);
        }
    }
}

pub(crate) fn host(node: &Node) -> Rc<dyn SlotHost<Style>> {
    Rc::new(ElementHost(Rc::downgrade(&node.0)))
}

fn style(
    declaration: &'static str,
    selector: &'static str,
    media: &'static str,
    children: Children<Style>,
) -> Style {
    let style = Style(Rc::new(StyleData {
        declaration,
        selector,
        media,
        children: RefCell::default(),
    }));
    children.mount(Rc::new(StyleHost(Rc::downgrade(&style.0))) as Rc<dyn SlotHost<Style>>);
    style
}

#[component]
pub fn Styles() -> Node {
    Node::new(Kind::Styles)
}

macro_rules! utilities {
    ($($name:ident => $css:literal),* $(,)?) => {$ (
        #[component]
        pub fn $name() -> Style {
            style($css, "", "", Children::default())
        }
    )*};
}

utilities! {
    Block => "display:block;",
    Flex => "display:flex;",
    InlineFlex => "display:inline-flex;",
    Grid => "display:grid;",
    Hidden => "display:none;",
    FlexCol => "flex-direction:column;",
    FlexRow => "flex-direction:row;",
    ItemsCenter => "align-items:center;",
    JustifyCenter => "justify-content:center;",
    Gap2 => "gap:.5rem;",
    Gap3 => "gap:.75rem;",
    Gap4 => "gap:1rem;",
    P0 => "padding:0;",
    P4 => "padding:1rem;",
    P8 => "padding:2rem;",
    Px4 => "padding-left:1rem;padding-right:1rem;",
    Py2 => "padding-top:.5rem;padding-bottom:.5rem;",
    M0 => "margin:0;",
    MxAuto => "margin-left:auto;margin-right:auto;",
    My4 => "margin-top:1rem;margin-bottom:1rem;",
    My8 => "margin-top:2rem;margin-bottom:2rem;",
    My12 => "margin-top:3rem;margin-bottom:3rem;",
    WFull => "width:100%;",
    HFull => "height:100%;",
    MaxW112 => "max-width:28rem;",
    MaxW160 => "max-width:40rem;",
    MaxW240 => "max-width:60rem;",
    Relative => "position:relative;",
    Absolute => "position:absolute;",
    Fixed => "position:fixed;",
    Inset0 => "inset:0;",
    Left0 => "left:0;",
    Top0 => "top:0;",
    Z1 => "z-index:1;",
    OriginTopLeft => "transform-origin:0 0;",
    OverflowHidden => "overflow:hidden;",
    PointerEventsNone => "pointer-events:none;",
    CursorPointer => "cursor:pointer;",
    CursorNotAllowed => "cursor:not-allowed;",
    TextCenter => "text-align:center;",
    TextLeft => "text-align:left;",
    AlignTop => "vertical-align:top;",
    WhitespacePreWrap => "white-space:pre-wrap;",
    BreakAll => "word-break:break-all;",
    BorderCollapse => "border-collapse:collapse;",
    BoxBorder => "box-sizing:border-box;",
    FontSans => "font-family:system-ui,sans-serif;",
    FontInherit => "font:inherit;",
    TextBase => "font-size:1rem;",
    TextSm => "font-size:.875rem;",
    TextXs => "font-size:.75rem;",
    FontMedium => "font-weight:500;",
    RoundedMd => "border-radius:.375rem;",
    Underline => "text-decoration:underline;",
    Opacity50 => "opacity:.5;",
    Ring2 => "outline:2px solid currentColor;outline-offset:2px;",
    ColorSchemeLightDark => "color-scheme:light dark;",
    BgSurface => "background:var(--bht-surface,#1b1b1b);",
    TextForeground => "color:var(--bht-foreground,#d0d0d0);",
    TextMuted => "color:var(--bht-muted,#9a9a9a);",
    BgPaper => "background:var(--bht-paper,#fafafa);",
    TextInk => "color:var(--bht-ink,#202124);",
    TextDanger => "color:var(--bht-danger,#a00);",
    BgAccent => "background:var(--bht-accent,#2563eb);",
    BgAccentHover => "background:var(--bht-accent-hover,#1d4ed8);",
    TextOnAccent => "color:var(--bht-on-accent,#fff);"
}

#[derive(Clone, Copy)]
pub enum Spacing {
    Control,
    Compact,
    Table,
}

#[component]
pub fn Padding(size: Spacing) -> Style {
    style(
        match size {
            Spacing::Control => "padding:.7rem;",
            Spacing::Compact => "padding:.4rem;",
            Spacing::Table => "padding:.4rem .8rem .4rem 0;",
        },
        "",
        "",
        Children::default(),
    )
}

#[component]
pub fn ControlMargin() -> Style {
    style("margin:.4rem .4rem .4rem 0;", "", "", Children::default())
}

macro_rules! variants {
    ($($name:ident => ($selector:literal, $media:literal)),* $(,)?) => {$ (
        #[component]
        pub fn $name(children: Children<Style>) -> Style {
            style("", $selector, $media, children)
        }
    )*};
}

variants! {
    Hover => (":hover", ""),
    FocusVisible => (":focus-visible", ""),
    Disabled => (":disabled", ""),
    WhenHidden => ("[hidden]", ""),
    Sm => ("", "(min-width:40rem)"),
    Md => ("", "(min-width:48rem)"),
    Lg => ("", "(min-width:64rem)")
}

#[derive(Clone, Copy)]
pub enum Target {
    Canvas,
    Label,
    Input,
    Button,
    Alert,
    Cell,
}

#[component]
pub fn Descendants(target: Target, children: Children<Style>) -> Style {
    style(
        "",
        match target {
            Target::Canvas => " canvas",
            Target::Label => " label",
            Target::Input => " input",
            Target::Button => " button",
            Target::Alert => " [role=alert]",
            Target::Cell => " :is(td,th)",
        },
        "",
        children,
    )
}

fn rules(styles: &ChildList<Style>, selector: &str, media: &[&str], output: &mut String) {
    for style in styles.iter() {
        let data = &style.0;
        let selector = format!("{selector}{}", data.selector);
        let mut media = media.to_vec();
        if !data.media.is_empty() {
            media.push(data.media);
        }
        if !data.declaration.is_empty() {
            for query in &media {
                output.push_str(&format!("@media {query}{{"));
            }
            output.push('&');
            output.push_str(&selector);
            output.push('{');
            output.push_str(data.declaration);
            output.push('}');
            for _ in &media {
                output.push('}');
            }
        }
        rules(&data.children.borrow(), &selector, &media, output);
    }
}

fn key(styles: &ChildList<Style>) -> String {
    let mut result = String::new();
    rules(styles, "", &[], &mut result);
    result
}

fn name(key: &str) -> String {
    let hash = key.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("bht-{hash:016x}")
}

pub(crate) fn class_name(styles: &ChildList<Style>) -> Option<String> {
    let key = key(styles);
    (!key.is_empty()).then(|| name(&key))
}

#[derive(Default)]
pub(crate) struct Sheet(BTreeMap<String, String>);
impl Sheet {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn add(&mut self, styles: &ChildList<Style>) {
        let key = key(styles);
        if !key.is_empty() {
            let name = name(&key);
            if let Some(previous) = self.0.insert(name, key.clone()) {
                assert_eq!(previous, key, "CSS class hash collision");
            }
        }
    }
    pub fn write(&self, output: &mut String) {
        for (name, key) in &self.0 {
            output.push_str(&key.replace('&', &format!(".{name}")));
        }
    }
}
