use accesskit::{Node, Role};
use beui_core::color::{Color32, Hsva, format_hex, parse_hex};
use beui_view::reactive::{
    Callback, Memo, Prop, ReadSignal, WriteSignal, clone, create_effect, create_memo, create_signal,
};

#[derive(Clone)]
pub struct ColorPickerState {
    color: ReadSignal<Hsva>,
    set_color: WriteSignal<Hsva>,
    reported: ReadSignal<Color32>,
    set_reported: WriteSignal<Color32>,
    dragging: ReadSignal<bool>,
    set_dragging: WriteSignal<bool>,
    disabled: Memo<bool>,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
}

impl ColorPickerState {
    pub fn new(
        value: Prop<Color32>,
        disabled: Prop<bool>,
        on_change: Callback<Color32>,
        on_preview: Callback<Option<Color32>>,
    ) -> Self {
        let initial = value.peek();
        let (color, set_color) = create_signal(Hsva::from_color(initial));
        let (reported, set_reported) = create_signal(initial);
        let (dragging, set_dragging) = create_signal(false);
        create_effect(clone!(color set_color set_reported -> move || {
            let next = value.get();
            set_reported.set(next);
            let held = color.get_untracked();
            set_color.set(Hsva::from_color_keeping(next, held));
        }));
        Self {
            color,
            set_color,
            reported,
            set_reported,
            dragging,
            set_dragging,
            disabled: create_memo(move || disabled.get()),
            on_change,
            on_preview,
        }
    }

    pub fn color(&self) -> ReadSignal<Hsva> {
        self.color.clone()
    }

    pub fn shown(&self) -> Memo<Color32> {
        let color = self.color.clone();
        create_memo(move || color.get().to_color())
    }

    pub fn disabled(&self) -> Memo<bool> {
        self.disabled.clone()
    }

    pub fn apply(&self, next: Hsva) {
        if self.disabled.get_untracked() {
            return;
        }
        self.set_color.set(next);
        let color = next.to_color();
        if self.dragging.get_untracked() {
            self.on_preview.call(Some(color));
        } else {
            self.report(color);
        }
    }

    pub fn drag(&self, dragging: bool) {
        self.set_dragging.set(dragging);
        if !dragging {
            self.report(self.color.get_untracked().to_color());
            self.on_preview.call(None);
        }
    }

    pub fn pick(&self, color: Color32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::from_color_keeping(color, held));
    }

    pub fn set_hue(&self, hue: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::new(
            hue.min(359.9),
            held.saturation,
            held.value,
            held.alpha,
        ));
    }

    pub fn set_alpha(&self, alpha: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva { alpha, ..held });
    }

    pub fn set_channel(&self, index: usize, value: f64) {
        let mut channels = self.color.get_untracked().to_color().to_array();
        channels[index] = value.round().clamp(0.0, 255.0) as u8;
        let [red, green, blue, alpha] = channels;
        self.pick(Color32::from_rgba_unmultiplied(red, green, blue, alpha));
    }

    pub fn set_saturation(&self, saturation: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::new(held.hue, saturation, held.value, held.alpha));
    }

    pub fn set_value(&self, value: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::new(held.hue, held.saturation, value, held.alpha));
    }

    pub fn hue_accessibility(&self) -> Memo<Node> {
        let color = self.color.clone();
        create_memo(move || {
            let mut node = Node::new(Role::Slider);
            node.set_label("Hue");
            node.set_value(format!("{} degrees", color.get().hue.round()));
            node
        })
    }

    pub fn alpha_accessibility(&self) -> Memo<Node> {
        let color = self.color.clone();
        create_memo(move || {
            let mut node = Node::new(Role::Slider);
            node.set_label("Opacity");
            node.set_value(format!("{}%", (color.get().alpha * 100.0).round()));
            node
        })
    }

    fn report(&self, color: Color32) {
        if color != self.reported.get_untracked() {
            self.set_reported.set(color);
            self.on_change.call(color);
        }
    }
}

#[derive(Clone)]
pub struct HexText {
    text: ReadSignal<String>,
    set_text: WriteSignal<String>,
    shown: Memo<Color32>,
    alpha: bool,
    apply: Callback<Color32>,
}

impl HexText {
    pub fn new(shown: Memo<Color32>, alpha: bool, apply: Callback<Color32>) -> Self {
        let (text, set_text) = create_signal(hex_text(shown.get_untracked(), alpha));
        create_effect(clone!(shown text set_text -> move || {
            let next = shown.get();
            if parse_typed(&text.get_untracked(), next) != Some(next) {
                set_text.set(hex_text(next, alpha));
            }
        }));
        Self {
            text,
            set_text,
            shown,
            alpha,
            apply,
        }
    }

    pub fn text(&self) -> ReadSignal<String> {
        self.text.clone()
    }

    pub fn placeholder(&self) -> &'static str {
        match self.alpha {
            true => "#RRGGBBAA",
            false => "#RRGGBB",
        }
    }

    pub fn edit(&self, typed: String) {
        self.set_text.set(typed.clone());
        if let Some(parsed) = parse_typed(&typed, self.shown.get_untracked()) {
            self.apply.call(parsed);
        }
    }

    pub fn submit(&self, typed: String) {
        match parse_hex(&typed) {
            Some(parsed) => {
                let held = self.shown.get_untracked();
                self.apply.call(keep_alpha(&typed, parsed, held));
            }
            None => self
                .set_text
                .set(hex_text(self.shown.get_untracked(), self.alpha)),
        }
    }
}

fn hex_text(color: Color32, alpha: bool) -> String {
    format_hex(color, alpha && color.alpha() < u8::MAX)
}

fn parse_typed(text: &str, held: Color32) -> Option<Color32> {
    let digits = text.trim().trim_start_matches('#');
    if digits.len() != 6 && digits.len() != 8 {
        return None;
    }
    parse_hex(text).map(|parsed| keep_alpha(text, parsed, held))
}

fn keep_alpha(text: &str, parsed: Color32, held: Color32) -> Color32 {
    let digits = text.trim().trim_start_matches('#').len();
    match digits {
        3 | 6 => {
            let [red, green, blue, _] = parsed.to_array();
            Color32::from_rgba_unmultiplied(red, green, blue, held.alpha())
        }
        _ => parsed,
    }
}
