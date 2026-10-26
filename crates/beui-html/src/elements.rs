use crate::{
    Callback, Children, Event, Kind, Node, Prop, bind_attribute, bind_flag, component, listen,
};

macro_rules! element {
    (normal $name:ident $tag:literal $strings:tt $numbers:tt $flags:tt) => {
        element!(@component $name $tag node [children: Children<Node>,] [children.mount(&node);] $strings $numbers $flags);
    };
    (void $name:ident $tag:literal $strings:tt $numbers:tt $flags:tt) => {
        element!(@component $name $tag node [] [] $strings $numbers $flags);
    };
    (@component $name:ident $tag:literal $node:ident [$($children:tt)*] [$($mount:tt)*]
        [$($string:ident => $string_name:literal),*]
        [$($number:ident: $number_type:ident => $number_name:literal),*]
        [$($flag:ident => $flag_name:literal),*]) => {
        #[component]
        pub fn $name(
            #[prop(default = Children::default())] css: Children<crate::tw::Style>,
            #[prop(default = None)] id: Prop<Option<String>>,
            #[prop(default = None)] class: Prop<Option<String>>,
            #[prop(default = None)] style: Prop<Option<String>>,
            #[prop(default = None)] title: Prop<Option<String>>,
            #[prop(default = None)] lang: Prop<Option<String>>,
            #[prop(default = None)] dir: Prop<Option<String>>,
            #[prop(default = None)] role: Prop<Option<String>>,
            #[prop(default = None)] tabindex: Prop<Option<i32>>,
            #[prop(default = false)] hidden: Prop<bool>,
            #[prop(default = false)] inert: Prop<bool>,
            on_click: Callback<Event>,
            on_input: Callback<Event>,
            on_change: Callback<Event>,
            on_submit: Callback<Event>,
            on_keydown: Callback<Event>,
            on_keyup: Callback<Event>,
            on_focus: Callback<Event>,
            on_blur: Callback<Event>,
            on_pointerdown: Callback<Event>,
            on_pointermove: Callback<Event>,
            on_pointerup: Callback<Event>,
            $(#[prop(default = None)] $string: Prop<Option<String>> ,)*
            $(#[prop(default = None)] $number: Prop<Option<$number_type>> ,)*
            $(#[prop(default = false)] $flag: Prop<bool> ,)*
            $($children)*
        ) -> Node {
            let $node = Node::new(Kind::Element($tag));
            css.mount(crate::tw::host(&$node));
            bind_attribute(&$node, "id", id);
            bind_attribute(&$node, "class", class);
            bind_attribute(&$node, "style", style);
            bind_attribute(&$node, "title", title);
            bind_attribute(&$node, "lang", lang);
            bind_attribute(&$node, "dir", dir);
            bind_attribute(&$node, "role", role);
            bind_attribute(&$node, "tabindex", tabindex);
            bind_flag(&$node, "hidden", hidden);
            bind_flag(&$node, "inert", inert);
            listen(&$node, "click", on_click);
            listen(&$node, "input", on_input);
            listen(&$node, "change", on_change);
            listen(&$node, "submit", on_submit);
            listen(&$node, "keydown", on_keydown);
            listen(&$node, "keyup", on_keyup);
            listen(&$node, "focus", on_focus);
            listen(&$node, "blur", on_blur);
            listen(&$node, "pointerdown", on_pointerdown);
            listen(&$node, "pointermove", on_pointermove);
            listen(&$node, "pointerup", on_pointerup);
            $(bind_attribute(&$node, $string_name, $string);)*
            $(bind_attribute(&$node, $number_name, $number);)*
            $(bind_flag(&$node, $flag_name, $flag);)*
            $($mount)*
            $node
        }
    };
}

element!(normal Html "html" [] [] []);
element!(normal Head "head" [] [] []);
element!(normal Title "title" [] [] []);
element!(normal Body "body" [] [] []);
element!(void Base "base" [href => "href", target => "target"] [] []);
element!(void Meta "meta" [charset => "charset", name => "name", content => "content", http_equiv => "http-equiv"] [] []);
element!(void Link "link" [href => "href", rel => "rel", r#type => "type", media => "media", crossorigin => "crossorigin", integrity => "integrity", referrerpolicy => "referrerpolicy", sizes => "sizes", r#as => "as"] [] [disabled => "disabled"]);
element!(normal Style "style" [media => "media", r#type => "type"] [] []);
element!(normal Script "script" [src => "src", r#type => "type", crossorigin => "crossorigin", integrity => "integrity", referrerpolicy => "referrerpolicy"] [] [r#async => "async", defer => "defer", nomodule => "nomodule"]);
element!(normal A "a" [href => "href", target => "target", rel => "rel", download => "download", hreflang => "hreflang", r#type => "type", referrerpolicy => "referrerpolicy"] [] []);
element!(normal Form "form" [action => "action", method => "method", enctype => "enctype", target => "target", autocomplete => "autocomplete", name => "name", rel => "rel"] [] [novalidate => "novalidate"]);
element!(normal Label "label" [r#for => "for", form => "form"] [] []);
element!(void Input "input" [r#type => "type", name => "name", value => "value", autocomplete => "autocomplete", placeholder => "placeholder", pattern => "pattern", min => "min", max => "max", step => "step", form => "form", formaction => "formaction", formenctype => "formenctype", formmethod => "formmethod", formtarget => "formtarget", accept => "accept", capture => "capture", list => "list", src => "src", alt => "alt"] [maxlength: u32 => "maxlength", minlength: u32 => "minlength", size: u32 => "size", width: u32 => "width", height: u32 => "height"] [required => "required", disabled => "disabled", checked => "checked", readonly => "readonly", multiple => "multiple", autofocus => "autofocus", formnovalidate => "formnovalidate"]);
element!(normal Button "button" [r#type => "type", name => "name", value => "value", form => "form", formaction => "formaction", formenctype => "formenctype", formmethod => "formmethod", formtarget => "formtarget"] [] [disabled => "disabled", autofocus => "autofocus", formnovalidate => "formnovalidate"]);
element!(normal Textarea "textarea" [name => "name", autocomplete => "autocomplete", placeholder => "placeholder", form => "form", wrap => "wrap"] [cols: u32 => "cols", rows: u32 => "rows", maxlength: u32 => "maxlength", minlength: u32 => "minlength"] [required => "required", disabled => "disabled", readonly => "readonly", autofocus => "autofocus"]);
element!(normal Select "select" [name => "name", autocomplete => "autocomplete", form => "form"] [size: u32 => "size"] [required => "required", disabled => "disabled", multiple => "multiple", autofocus => "autofocus"]);
element!(normal Option "option" [value => "value", label => "label"] [] [disabled => "disabled", selected => "selected"]);
element!(normal Optgroup "optgroup" [label => "label"] [] [disabled => "disabled"]);
element!(normal Fieldset "fieldset" [name => "name", form => "form"] [] [disabled => "disabled"]);
element!(normal Legend "legend" [] [] []);
element!(void Img "img" [src => "src", alt => "alt", srcset => "srcset", sizes => "sizes", loading => "loading", decoding => "decoding", crossorigin => "crossorigin", referrerpolicy => "referrerpolicy"] [width: u32 => "width", height: u32 => "height"] [ismap => "ismap"]);
element!(normal Canvas "canvas" [] [width: u32 => "width", height: u32 => "height"] []);
element!(normal Details "details" [name => "name"] [] [open => "open"]);
element!(normal Dialog "dialog" [] [] [open => "open"]);
element!(normal Td "td" [headers => "headers"] [colspan: u32 => "colspan", rowspan: u32 => "rowspan"] []);
element!(normal Th "th" [scope => "scope", abbr => "abbr", headers => "headers"] [colspan: u32 => "colspan", rowspan: u32 => "rowspan"] []);
element!(normal Ol "ol" [r#type => "type"] [start: i32 => "start"] [reversed => "reversed"]);
element!(normal Li "li" [] [value: i32 => "value"] []);
element!(normal Time "time" [datetime => "datetime"] [] []);
element!(normal Data "data" [value => "value"] [] []);
element!(normal Progress "progress" [] [value: f64 => "value", max: f64 => "max"] []);
element!(normal Meter "meter" [] [value: f64 => "value", min: f64 => "min", max: f64 => "max", low: f64 => "low", high: f64 => "high", optimum: f64 => "optimum"] []);
element!(normal Blockquote "blockquote" [cite => "cite"] [] []);
element!(normal Q "q" [cite => "cite"] [] []);
element!(normal Ins "ins" [cite => "cite", datetime => "datetime"] [] []);
element!(normal Del "del" [cite => "cite", datetime => "datetime"] [] []);
element!(normal Iframe "iframe" [src => "src", name => "name", srcdoc => "srcdoc", allow => "allow", loading => "loading", referrerpolicy => "referrerpolicy", sandbox => "sandbox"] [width: u32 => "width", height: u32 => "height"] [allowfullscreen => "allowfullscreen"]);
element!(normal Video "video" [src => "src", poster => "poster", preload => "preload", crossorigin => "crossorigin"] [width: u32 => "width", height: u32 => "height"] [controls => "controls", autoplay => "autoplay", r#loop => "loop", muted => "muted", playsinline => "playsinline"]);
element!(normal Audio "audio" [src => "src", preload => "preload", crossorigin => "crossorigin"] [] [controls => "controls", autoplay => "autoplay", r#loop => "loop", muted => "muted"]);
element!(void Source "source" [src => "src", srcset => "srcset", sizes => "sizes", r#type => "type", media => "media"] [width: u32 => "width", height: u32 => "height"] []);
element!(void Track "track" [src => "src", kind => "kind", srclang => "srclang", label => "label"] [] [default => "default"]);
element!(normal Output "output" [name => "name", form => "form", r#for => "for"] [] []);
element!(normal Main "main" [] [] []);
element!(normal Header "header" [] [] []);
element!(normal Footer "footer" [] [] []);
element!(normal Nav "nav" [] [] []);
element!(normal Section "section" [] [] []);
element!(normal Article "article" [] [] []);
element!(normal Aside "aside" [] [] []);
element!(normal Address "address" [] [] []);
element!(normal Div "div" [] [] []);
element!(normal Span "span" [] [] []);
element!(normal P "p" [] [] []);
element!(normal H1 "h1" [] [] []);
element!(normal H2 "h2" [] [] []);
element!(normal H3 "h3" [] [] []);
element!(normal H4 "h4" [] [] []);
element!(normal H5 "h5" [] [] []);
element!(normal H6 "h6" [] [] []);
element!(normal Strong "strong" [] [] []);
element!(normal Em "em" [] [] []);
element!(normal B "b" [] [] []);
element!(normal I "i" [] [] []);
element!(normal U "u" [] [] []);
element!(normal S "s" [] [] []);
element!(normal Small "small" [] [] []);
element!(normal Sub "sub" [] [] []);
element!(normal Sup "sup" [] [] []);
element!(normal Code "code" [] [] []);
element!(normal Pre "pre" [] [] []);
element!(normal Kbd "kbd" [] [] []);
element!(normal Samp "samp" [] [] []);
element!(normal Var "var" [] [] []);
element!(normal Mark "mark" [] [] []);
element!(normal Abbr "abbr" [] [] []);
element!(normal Cite "cite" [] [] []);
element!(normal Dfn "dfn" [] [] []);
element!(normal Bdi "bdi" [] [] []);
element!(normal Bdo "bdo" [] [] []);
element!(normal Ruby "ruby" [] [] []);
element!(normal Rt "rt" [] [] []);
element!(normal Rp "rp" [] [] []);
element!(void Wbr "wbr" [] [] []);
element!(void Br "br" [] [] []);
element!(void Hr "hr" [] [] []);
element!(normal Picture "picture" [] [] []);
element!(normal Figure "figure" [] [] []);
element!(normal Figcaption "figcaption" [] [] []);
element!(normal Summary "summary" [] [] []);
element!(normal Table "table" [] [] []);
element!(normal Caption "caption" [] [] []);
element!(normal Colgroup "colgroup" [] [] []);
element!(normal Thead "thead" [] [] []);
element!(normal Tbody "tbody" [] [] []);
element!(normal Tfoot "tfoot" [] [] []);
element!(normal Tr "tr" [] [] []);
element!(normal Ul "ul" [] [] []);
element!(normal Dl "dl" [] [] []);
element!(normal Dt "dt" [] [] []);
element!(normal Dd "dd" [] [] []);
element!(normal Menu "menu" [] [] []);
element!(normal Noscript "noscript" [] [] []);
element!(normal Template "template" [] [] []);
element!(normal Slot "slot" [] [] []);
element!(normal Search "search" [] [] []);
element!(normal Hgroup "hgroup" [] [] []);
element!(void Col "col" [] [span: u32 => "span"] []);

element!(normal Datalist "datalist" [] [] []);
element!(normal Map "map" [name => "name"] [] []);
element!(void Area "area" [href => "href", target => "target", rel => "rel", alt => "alt", coords => "coords", shape => "shape", download => "download", referrerpolicy => "referrerpolicy"] [] []);
element!(void Embed "embed" [src => "src", r#type => "type"] [width: u32 => "width", height: u32 => "height"] []);
element!(normal Object "object" [data => "data", r#type => "type", name => "name", form => "form"] [width: u32 => "width", height: u32 => "height"] []);
