use crate::accessibility as wire;

impl From<accesskit::ActionRequest> for wire::ActionRequest {
    fn from(request: accesskit::ActionRequest) -> Self {
        Self {
            action: action_to(request.action),
            target_tree: request.target_tree.0.into_bytes(),
            target_node: request.target_node.0,
            data: request.data.map(data_to),
        }
    }
}

impl From<wire::ActionRequest> for accesskit::ActionRequest {
    fn from(request: wire::ActionRequest) -> Self {
        Self {
            action: action_from(request.action),
            target_tree: accesskit::TreeId(uuid::Uuid::from_bytes(request.target_tree)),
            target_node: accesskit::NodeId(request.target_node),
            data: request.data.map(data_from),
        }
    }
}

macro_rules! actions {
    ($($name:ident),* $(,)?) => {
        fn action_to(action: accesskit::Action) -> wire::Action {
            match action {
                $(accesskit::Action::$name => wire::Action::$name,)*
            }
        }

        fn action_from(action: wire::Action) -> accesskit::Action {
            match action {
                $(wire::Action::$name => accesskit::Action::$name,)*
            }
        }
    };
}

macro_rules! all_actions {
    ($($name:ident),* $(,)?) => {
        const ACTIONS: &[accesskit::Action] = &[$(accesskit::Action::$name),*];
    };
}

all_actions!(
    Click,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    HideTooltip,
    ShowTooltip,
    ReplaceSelectedText,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint,
    SetScrollOffset,
    SetTextSelection,
    SetSequentialFocusNavigationStartingPoint,
    SetValue,
    ShowContextMenu,
);

actions!(
    Click,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    HideTooltip,
    ShowTooltip,
    ReplaceSelectedText,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint,
    SetScrollOffset,
    SetTextSelection,
    SetSequentialFocusNavigationStartingPoint,
    SetValue,
    ShowContextMenu,
);

fn position_to(position: accesskit::TextPosition) -> wire::TextPosition {
    wire::TextPosition {
        node: position.node.0,
        character_index: position.character_index as u64,
    }
}

fn position_from(position: wire::TextPosition) -> accesskit::TextPosition {
    accesskit::TextPosition {
        node: accesskit::NodeId(position.node),
        character_index: position.character_index as usize,
    }
}

fn data_to(data: accesskit::ActionData) -> wire::ActionData {
    use accesskit::{ActionData, ScrollHint, ScrollUnit};
    let point = |point: accesskit::Point| wire::Point {
        x: point.x,
        y: point.y,
    };
    match data {
        ActionData::CustomAction(id) => wire::ActionData::CustomAction(id),
        ActionData::Value(value) => wire::ActionData::Value(value.into()),
        ActionData::NumericValue(value) => wire::ActionData::NumericValue(value),
        ActionData::ScrollUnit(unit) => wire::ActionData::ScrollUnit(match unit {
            ScrollUnit::Item => wire::ScrollUnit::Item,
            ScrollUnit::Page => wire::ScrollUnit::Page,
        }),
        ActionData::ScrollHint(hint) => wire::ActionData::ScrollHint(match hint {
            ScrollHint::TopLeft => wire::ScrollHint::TopLeft,
            ScrollHint::BottomRight => wire::ScrollHint::BottomRight,
            ScrollHint::TopEdge => wire::ScrollHint::TopEdge,
            ScrollHint::BottomEdge => wire::ScrollHint::BottomEdge,
            ScrollHint::LeftEdge => wire::ScrollHint::LeftEdge,
            ScrollHint::RightEdge => wire::ScrollHint::RightEdge,
        }),
        ActionData::ScrollToPoint(target) => wire::ActionData::ScrollToPoint(point(target)),
        ActionData::SetScrollOffset(offset) => wire::ActionData::SetScrollOffset(point(offset)),
        ActionData::SetTextSelection(selection) => {
            wire::ActionData::SetTextSelection(wire::TextSelection {
                anchor: position_to(selection.anchor),
                focus: position_to(selection.focus),
            })
        }
    }
}

fn data_from(data: wire::ActionData) -> accesskit::ActionData {
    use accesskit::{ActionData, ScrollHint, ScrollUnit};
    let point = |point: wire::Point| accesskit::Point::new(point.x, point.y);
    match data {
        wire::ActionData::CustomAction(id) => ActionData::CustomAction(id),
        wire::ActionData::Value(value) => ActionData::Value(value.into()),
        wire::ActionData::NumericValue(value) => ActionData::NumericValue(value),
        wire::ActionData::ScrollUnit(unit) => ActionData::ScrollUnit(match unit {
            wire::ScrollUnit::Item => ScrollUnit::Item,
            wire::ScrollUnit::Page => ScrollUnit::Page,
        }),
        wire::ActionData::ScrollHint(hint) => ActionData::ScrollHint(match hint {
            wire::ScrollHint::TopLeft => ScrollHint::TopLeft,
            wire::ScrollHint::BottomRight => ScrollHint::BottomRight,
            wire::ScrollHint::TopEdge => ScrollHint::TopEdge,
            wire::ScrollHint::BottomEdge => ScrollHint::BottomEdge,
            wire::ScrollHint::LeftEdge => ScrollHint::LeftEdge,
            wire::ScrollHint::RightEdge => ScrollHint::RightEdge,
        }),
        wire::ActionData::ScrollToPoint(target) => ActionData::ScrollToPoint(point(target)),
        wire::ActionData::SetScrollOffset(offset) => ActionData::SetScrollOffset(point(offset)),
        wire::ActionData::SetTextSelection(selection) => {
            ActionData::SetTextSelection(accesskit::TextSelection {
                anchor: position_from(selection.anchor),
                focus: position_from(selection.focus),
            })
        }
    }
}

macro_rules! same_enum {
    ($to:ident, $from:ident, $theirs:path, $ours:path, [$($case:ident),* $(,)?]) => {
        fn $to(value: $theirs) -> $ours {
            use $theirs as Theirs;
            use $ours as Ours;
            match value {
                $(Theirs::$case => Ours::$case,)*
            }
        }

        fn $from(value: $ours) -> $theirs {
            use $theirs as Theirs;
            use $ours as Ours;
            match value {
                $(Ours::$case => Theirs::$case,)*
            }
        }
    };
}
same_enum!(
    role_to,
    role_from,
    accesskit::Role,
    wire::Role,
    [
        Unknown,
        TextRun,
        Cell,
        Label,
        Image,
        Link,
        Row,
        ListItem,
        ListMarker,
        TreeItem,
        ListBoxOption,
        MenuItem,
        MenuListOption,
        Paragraph,
        GenericContainer,
        CheckBox,
        RadioButton,
        TextInput,
        Button,
        DefaultButton,
        Pane,
        RowHeader,
        ColumnHeader,
        RowGroup,
        List,
        Table,
        LayoutTableCell,
        LayoutTableRow,
        LayoutTable,
        Switch,
        Menu,
        MultilineTextInput,
        SearchInput,
        DateInput,
        DateTimeInput,
        WeekInput,
        MonthInput,
        TimeInput,
        EmailInput,
        NumberInput,
        PasswordInput,
        PhoneNumberInput,
        UrlInput,
        Abbr,
        Alert,
        AlertDialog,
        Application,
        Article,
        Audio,
        Banner,
        Blockquote,
        Canvas,
        Caption,
        Caret,
        Code,
        ColorWell,
        ComboBox,
        EditableComboBox,
        Complementary,
        Comment,
        ContentDeletion,
        ContentInsertion,
        ContentInfo,
        Definition,
        DescriptionList,
        Details,
        Dialog,
        DisclosureTriangle,
        Document,
        EmbeddedObject,
        Emphasis,
        Feed,
        FigureCaption,
        Figure,
        Footer,
        Form,
        Grid,
        GridCell,
        Group,
        Header,
        Heading,
        Iframe,
        IframePresentational,
        ImeCandidate,
        Keyboard,
        Legend,
        LineBreak,
        ListBox,
        Log,
        Main,
        Mark,
        Marquee,
        Math,
        MenuBar,
        MenuItemCheckBox,
        MenuItemRadio,
        MenuListPopup,
        Meter,
        Navigation,
        Note,
        PluginObject,
        ProgressIndicator,
        RadioGroup,
        Region,
        RootWebArea,
        Ruby,
        RubyAnnotation,
        ScrollBar,
        ScrollView,
        Search,
        Section,
        SectionFooter,
        SectionHeader,
        Slider,
        SpinButton,
        Splitter,
        Status,
        Strong,
        Suggestion,
        SvgRoot,
        Tab,
        TabList,
        TabPanel,
        Term,
        Time,
        Timer,
        TitleBar,
        Toolbar,
        Tooltip,
        Tree,
        TreeGrid,
        Video,
        WebView,
        Window,
        PdfActionableHighlight,
        PdfRoot,
        GraphicsDocument,
        GraphicsObject,
        GraphicsSymbol,
        DocAbstract,
        DocAcknowledgements,
        DocAfterword,
        DocAppendix,
        DocBackLink,
        DocBiblioEntry,
        DocBibliography,
        DocBiblioRef,
        DocChapter,
        DocColophon,
        DocConclusion,
        DocCover,
        DocCredit,
        DocCredits,
        DocDedication,
        DocEndnote,
        DocEndnotes,
        DocEpigraph,
        DocEpilogue,
        DocErrata,
        DocExample,
        DocFootnote,
        DocForeword,
        DocGlossary,
        DocGlossRef,
        DocIndex,
        DocIntroduction,
        DocNoteRef,
        DocNotice,
        DocPageBreak,
        DocPageFooter,
        DocPageHeader,
        DocPageList,
        DocPart,
        DocPreface,
        DocPrologue,
        DocPullquote,
        DocQna,
        DocSubtitle,
        DocTip,
        DocToc,
        ListGrid,
        Terminal
    ]
);
same_enum!(
    invalid_to,
    invalid_from,
    accesskit::Invalid,
    wire::Invalid,
    [True, Grammar, Spelling]
);
same_enum!(
    toggled_to,
    toggled_from,
    accesskit::Toggled,
    wire::Toggled,
    [False, True, Mixed]
);
same_enum!(
    live_to,
    live_from,
    accesskit::Live,
    wire::Live,
    [Off, Polite, Assertive]
);
same_enum!(
    text_direction_to,
    text_direction_from,
    accesskit::TextDirection,
    wire::TextDirection,
    [LeftToRight, RightToLeft, TopToBottom, BottomToTop]
);
same_enum!(
    orientation_to,
    orientation_from,
    accesskit::Orientation,
    wire::Orientation,
    [Horizontal, Vertical]
);
same_enum!(
    sort_direction_to,
    sort_direction_from,
    accesskit::SortDirection,
    wire::SortDirection,
    [Ascending, Descending, Other]
);
same_enum!(
    aria_current_to,
    aria_current_from,
    accesskit::AriaCurrent,
    wire::AriaCurrent,
    [False, True, Page, Step, Location, Date, Time]
);
same_enum!(
    auto_complete_to,
    auto_complete_from,
    accesskit::AutoComplete,
    wire::AutoComplete,
    [Inline, List, Both]
);
same_enum!(
    has_popup_to,
    has_popup_from,
    accesskit::HasPopup,
    wire::HasPopup,
    [Menu, Listbox, Tree, Grid, Dialog]
);
same_enum!(
    list_style_to,
    list_style_from,
    accesskit::ListStyle,
    wire::ListStyle,
    [Circle, Disc, Image, Numeric, Square, Other]
);
same_enum!(
    text_align_to,
    text_align_from,
    accesskit::TextAlign,
    wire::TextAlign,
    [Left, Right, Center, Justify]
);
same_enum!(
    vertical_offset_to,
    vertical_offset_from,
    accesskit::VerticalOffset,
    wire::VerticalOffset,
    [Subscript, Superscript]
);
same_enum!(
    text_decoration_style_to,
    text_decoration_style_from,
    accesskit::TextDecorationStyle,
    wire::TextDecorationStyle,
    [Solid, Dotted, Dashed, Double, Wavy]
);

fn node_to(id: accesskit::NodeId, node: &accesskit::Node) -> wire::Node {
    let mut properties = Vec::new();
    if !node.children().is_empty() {
        properties.push(wire::NodeProperty::Children(
            node.children().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.controls().is_empty() {
        properties.push(wire::NodeProperty::Controls(
            node.controls().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.details().is_empty() {
        properties.push(wire::NodeProperty::Details(
            node.details().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.described_by().is_empty() {
        properties.push(wire::NodeProperty::DescribedBy(
            node.described_by().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.flow_to().is_empty() {
        properties.push(wire::NodeProperty::FlowTo(
            node.flow_to().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.labelled_by().is_empty() {
        properties.push(wire::NodeProperty::LabelledBy(
            node.labelled_by().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.owns().is_empty() {
        properties.push(wire::NodeProperty::Owns(
            node.owns().iter().map(|id| id.0).collect(),
        ));
    }
    if !node.radio_group().is_empty() {
        properties.push(wire::NodeProperty::RadioGroup(
            node.radio_group().iter().map(|id| id.0).collect(),
        ));
    }
    if let Some(id) = node.active_descendant() {
        properties.push(wire::NodeProperty::ActiveDescendant(id.0));
    }
    if let Some(id) = node.error_message() {
        properties.push(wire::NodeProperty::ErrorMessage(id.0));
    }
    if let Some(id) = node.in_page_link_target() {
        properties.push(wire::NodeProperty::InPageLinkTarget(id.0));
    }
    if let Some(id) = node.member_of() {
        properties.push(wire::NodeProperty::MemberOf(id.0));
    }
    if let Some(id) = node.next_on_line() {
        properties.push(wire::NodeProperty::NextOnLine(id.0));
    }
    if let Some(id) = node.previous_on_line() {
        properties.push(wire::NodeProperty::PreviousOnLine(id.0));
    }
    if let Some(id) = node.popup_for() {
        properties.push(wire::NodeProperty::PopupFor(id.0));
    }
    if let Some(value) = node.label() {
        properties.push(wire::NodeProperty::Label(value.to_owned()));
    }
    if let Some(value) = node.description() {
        properties.push(wire::NodeProperty::Description(value.to_owned()));
    }
    if let Some(value) = node.value() {
        properties.push(wire::NodeProperty::Value(value.to_owned()));
    }
    if let Some(value) = node.access_key() {
        properties.push(wire::NodeProperty::AccessKey(value.to_owned()));
    }
    if let Some(value) = node.author_id() {
        properties.push(wire::NodeProperty::AuthorId(value.to_owned()));
    }
    if let Some(value) = node.class_name() {
        properties.push(wire::NodeProperty::ClassName(value.to_owned()));
    }
    if let Some(value) = node.font_family() {
        properties.push(wire::NodeProperty::FontFamily(value.to_owned()));
    }
    if let Some(value) = node.html_tag() {
        properties.push(wire::NodeProperty::HtmlTag(value.to_owned()));
    }
    if let Some(value) = node.inner_html() {
        properties.push(wire::NodeProperty::InnerHtml(value.to_owned()));
    }
    if let Some(value) = node.keyboard_shortcut() {
        properties.push(wire::NodeProperty::KeyboardShortcut(value.to_owned()));
    }
    if let Some(value) = node.language() {
        properties.push(wire::NodeProperty::Language(value.to_owned()));
    }
    if let Some(value) = node.placeholder() {
        properties.push(wire::NodeProperty::Placeholder(value.to_owned()));
    }
    if let Some(value) = node.role_description() {
        properties.push(wire::NodeProperty::RoleDescription(value.to_owned()));
    }
    if let Some(value) = node.state_description() {
        properties.push(wire::NodeProperty::StateDescription(value.to_owned()));
    }
    if let Some(value) = node.tooltip() {
        properties.push(wire::NodeProperty::Tooltip(value.to_owned()));
    }
    if let Some(value) = node.url() {
        properties.push(wire::NodeProperty::Url(value.to_owned()));
    }
    if let Some(value) = node.row_index_text() {
        properties.push(wire::NodeProperty::RowIndexText(value.to_owned()));
    }
    if let Some(value) = node.column_index_text() {
        properties.push(wire::NodeProperty::ColumnIndexText(value.to_owned()));
    }
    if let Some(value) = node.braille_label() {
        properties.push(wire::NodeProperty::BrailleLabel(value.to_owned()));
    }
    if let Some(value) = node.braille_role_description() {
        properties.push(wire::NodeProperty::BrailleRoleDescription(value.to_owned()));
    }
    if let Some(value) = node.scroll_x() {
        properties.push(wire::NodeProperty::ScrollX(value));
    }
    if let Some(value) = node.scroll_x_min() {
        properties.push(wire::NodeProperty::ScrollXMin(value));
    }
    if let Some(value) = node.scroll_x_max() {
        properties.push(wire::NodeProperty::ScrollXMax(value));
    }
    if let Some(value) = node.scroll_y() {
        properties.push(wire::NodeProperty::ScrollY(value));
    }
    if let Some(value) = node.scroll_y_min() {
        properties.push(wire::NodeProperty::ScrollYMin(value));
    }
    if let Some(value) = node.scroll_y_max() {
        properties.push(wire::NodeProperty::ScrollYMax(value));
    }
    if let Some(value) = node.numeric_value() {
        properties.push(wire::NodeProperty::NumericValue(value));
    }
    if let Some(value) = node.min_numeric_value() {
        properties.push(wire::NodeProperty::MinNumericValue(value));
    }
    if let Some(value) = node.max_numeric_value() {
        properties.push(wire::NodeProperty::MaxNumericValue(value));
    }
    if let Some(value) = node.numeric_value_step() {
        properties.push(wire::NodeProperty::NumericValueStep(value));
    }
    if let Some(value) = node.numeric_value_jump() {
        properties.push(wire::NodeProperty::NumericValueJump(value));
    }
    if let Some(value) = node.font_size() {
        properties.push(wire::NodeProperty::FontSize(value));
    }
    if let Some(value) = node.font_weight() {
        properties.push(wire::NodeProperty::FontWeight(value));
    }
    if let Some(value) = node.is_expanded() {
        properties.push(wire::NodeProperty::Expanded(value));
    }
    if let Some(value) = node.is_selected() {
        properties.push(wire::NodeProperty::Selected(value));
    }
    if let Some(value) = node.row_count() {
        properties.push(wire::NodeProperty::RowCount(value as u64));
    }
    if let Some(value) = node.column_count() {
        properties.push(wire::NodeProperty::ColumnCount(value as u64));
    }
    if let Some(value) = node.row_index() {
        properties.push(wire::NodeProperty::RowIndex(value as u64));
    }
    if let Some(value) = node.column_index() {
        properties.push(wire::NodeProperty::ColumnIndex(value as u64));
    }
    if let Some(value) = node.row_span() {
        properties.push(wire::NodeProperty::RowSpan(value as u64));
    }
    if let Some(value) = node.column_span() {
        properties.push(wire::NodeProperty::ColumnSpan(value as u64));
    }
    if let Some(value) = node.level() {
        properties.push(wire::NodeProperty::Level(value as u64));
    }
    if let Some(value) = node.size_of_set() {
        properties.push(wire::NodeProperty::SizeOfSet(value as u64));
    }
    if let Some(value) = node.position_in_set() {
        properties.push(wire::NodeProperty::PositionInSet(value as u64));
    }
    if let Some(value) = node.color_value() {
        properties.push(wire::NodeProperty::ColorValue(color_to(value)));
    }
    if let Some(value) = node.background_color() {
        properties.push(wire::NodeProperty::BackgroundColor(color_to(value)));
    }
    if let Some(value) = node.foreground_color() {
        properties.push(wire::NodeProperty::ForegroundColor(color_to(value)));
    }
    if let Some(value) = node.overline() {
        properties.push(wire::NodeProperty::Overline(wire::TextDecoration {
            style: text_decoration_style_to(value.style),
            color: color_to(value.color),
        }));
    }
    if let Some(value) = node.strikethrough() {
        properties.push(wire::NodeProperty::Strikethrough(wire::TextDecoration {
            style: text_decoration_style_to(value.style),
            color: color_to(value.color),
        }));
    }
    if let Some(value) = node.underline() {
        properties.push(wire::NodeProperty::Underline(wire::TextDecoration {
            style: text_decoration_style_to(value.style),
            color: color_to(value.color),
        }));
    }
    if !node.character_lengths().is_empty() {
        properties.push(wire::NodeProperty::CharacterLengths(
            node.character_lengths().to_vec(),
        ));
    }
    if !node.word_starts().is_empty() {
        properties.push(wire::NodeProperty::WordStarts(node.word_starts().to_vec()));
    }
    if let Some(value) = node.character_positions() {
        properties.push(wire::NodeProperty::CharacterPositions(value.to_vec()));
    }
    if let Some(value) = node.character_widths() {
        properties.push(wire::NodeProperty::CharacterWidths(value.to_vec()));
    }
    if let Some(value) = node.invalid() {
        properties.push(wire::NodeProperty::Invalid(invalid_to(value)));
    }
    if let Some(value) = node.toggled() {
        properties.push(wire::NodeProperty::Toggled(toggled_to(value)));
    }
    if let Some(value) = node.live() {
        properties.push(wire::NodeProperty::Live(live_to(value)));
    }
    if let Some(value) = node.text_direction() {
        properties.push(wire::NodeProperty::TextDirection(text_direction_to(value)));
    }
    if let Some(value) = node.orientation() {
        properties.push(wire::NodeProperty::Orientation(orientation_to(value)));
    }
    if let Some(value) = node.sort_direction() {
        properties.push(wire::NodeProperty::SortDirection(sort_direction_to(value)));
    }
    if let Some(value) = node.aria_current() {
        properties.push(wire::NodeProperty::AriaCurrent(aria_current_to(value)));
    }
    if let Some(value) = node.auto_complete() {
        properties.push(wire::NodeProperty::AutoComplete(auto_complete_to(value)));
    }
    if let Some(value) = node.has_popup() {
        properties.push(wire::NodeProperty::HasPopup(has_popup_to(value)));
    }
    if let Some(value) = node.list_style() {
        properties.push(wire::NodeProperty::ListStyle(list_style_to(value)));
    }
    if let Some(value) = node.text_align() {
        properties.push(wire::NodeProperty::TextAlign(text_align_to(value)));
    }
    if let Some(value) = node.vertical_offset() {
        properties.push(wire::NodeProperty::VerticalOffset(vertical_offset_to(
            value,
        )));
    }
    if let Some(transform) = node.transform() {
        properties.push(wire::NodeProperty::Transform(
            transform.as_coeffs().to_vec(),
        ));
    }
    if let Some(bounds) = node.bounds() {
        properties.push(wire::NodeProperty::Bounds(wire::NodeRect {
            x0: bounds.x0,
            y0: bounds.y0,
            x1: bounds.x1,
            y1: bounds.y1,
        }));
    }
    if let Some(selection) = node.text_selection() {
        properties.push(wire::NodeProperty::TextSelection(wire::TextSelection {
            anchor: position_to(selection.anchor),
            focus: position_to(selection.focus),
        }));
    }
    if let Some(tree) = node.tree_id() {
        properties.push(wire::NodeProperty::TreeId(tree.0.into_bytes()));
    }
    if !node.custom_actions().is_empty() {
        properties.push(wire::NodeProperty::CustomActions(
            node.custom_actions()
                .iter()
                .map(|action| wire::CustomAction {
                    id: action.id,
                    description: action.description.to_string(),
                })
                .collect(),
        ));
    }
    let mut flags = Vec::new();
    if node.is_hidden() {
        flags.push(wire::NodeFlag::Hidden);
    }
    if node.is_multiselectable() {
        flags.push(wire::NodeFlag::Multiselectable);
    }
    if node.is_required() {
        flags.push(wire::NodeFlag::Required);
    }
    if node.is_visited() {
        flags.push(wire::NodeFlag::Visited);
    }
    if node.is_busy() {
        flags.push(wire::NodeFlag::Busy);
    }
    if node.is_live_atomic() {
        flags.push(wire::NodeFlag::LiveAtomic);
    }
    if node.is_modal() {
        flags.push(wire::NodeFlag::Modal);
    }
    if node.is_touch_transparent() {
        flags.push(wire::NodeFlag::TouchTransparent);
    }
    if node.is_read_only() {
        flags.push(wire::NodeFlag::ReadOnly);
    }
    if node.is_disabled() {
        flags.push(wire::NodeFlag::Disabled);
    }
    if node.is_italic() {
        flags.push(wire::NodeFlag::Italic);
    }
    if node.clips_children() {
        flags.push(wire::NodeFlag::ClipsChildren);
    }
    if node.is_line_breaking_object() {
        flags.push(wire::NodeFlag::IsLineBreakingObject);
    }
    if node.is_page_breaking_object() {
        flags.push(wire::NodeFlag::IsPageBreakingObject);
    }
    if node.is_spelling_error() {
        flags.push(wire::NodeFlag::IsSpellingError);
    }
    if node.is_grammar_error() {
        flags.push(wire::NodeFlag::IsGrammarError);
    }
    if node.is_search_match() {
        flags.push(wire::NodeFlag::IsSearchMatch);
    }
    if node.is_suggestion() {
        flags.push(wire::NodeFlag::IsSuggestion);
    }
    wire::Node {
        id: id.0,
        role: role_to(node.role()),
        actions: ACTIONS
            .iter()
            .copied()
            .filter(|action| node.supports_action(*action))
            .map(action_to)
            .collect(),
        flags,
        properties,
    }
}
fn node_from(node: wire::Node) -> (accesskit::NodeId, accesskit::Node) {
    let mut built = accesskit::Node::new(role_from(node.role));
    for action in node.actions {
        built.add_action(action_from(action));
    }
    for flag in node.flags {
        match flag {
            wire::NodeFlag::Hidden => built.set_hidden(),
            wire::NodeFlag::Multiselectable => built.set_multiselectable(),
            wire::NodeFlag::Required => built.set_required(),
            wire::NodeFlag::Visited => built.set_visited(),
            wire::NodeFlag::Busy => built.set_busy(),
            wire::NodeFlag::LiveAtomic => built.set_live_atomic(),
            wire::NodeFlag::Modal => built.set_modal(),
            wire::NodeFlag::TouchTransparent => built.set_touch_transparent(),
            wire::NodeFlag::ReadOnly => built.set_read_only(),
            wire::NodeFlag::Disabled => built.set_disabled(),
            wire::NodeFlag::Italic => built.set_italic(),
            wire::NodeFlag::ClipsChildren => built.set_clips_children(),
            wire::NodeFlag::IsLineBreakingObject => built.set_is_line_breaking_object(),
            wire::NodeFlag::IsPageBreakingObject => built.set_is_page_breaking_object(),
            wire::NodeFlag::IsSpellingError => built.set_is_spelling_error(),
            wire::NodeFlag::IsGrammarError => built.set_is_grammar_error(),
            wire::NodeFlag::IsSearchMatch => built.set_is_search_match(),
            wire::NodeFlag::IsSuggestion => built.set_is_suggestion(),
        }
    }
    for property in node.properties {
        match property {
            wire::NodeProperty::Children(ids) => {
                built.set_children(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::Controls(ids) => {
                built.set_controls(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::Details(ids) => {
                built.set_details(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::DescribedBy(ids) => {
                built.set_described_by(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::FlowTo(ids) => {
                built.set_flow_to(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::LabelledBy(ids) => {
                built.set_labelled_by(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::Owns(ids) => {
                built.set_owns(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::RadioGroup(ids) => {
                built.set_radio_group(ids.into_iter().map(accesskit::NodeId).collect::<Vec<_>>())
            }
            wire::NodeProperty::ActiveDescendant(id) => {
                built.set_active_descendant(accesskit::NodeId(id))
            }
            wire::NodeProperty::ErrorMessage(id) => built.set_error_message(accesskit::NodeId(id)),
            wire::NodeProperty::InPageLinkTarget(id) => {
                built.set_in_page_link_target(accesskit::NodeId(id))
            }
            wire::NodeProperty::MemberOf(id) => built.set_member_of(accesskit::NodeId(id)),
            wire::NodeProperty::NextOnLine(id) => built.set_next_on_line(accesskit::NodeId(id)),
            wire::NodeProperty::PreviousOnLine(id) => {
                built.set_previous_on_line(accesskit::NodeId(id))
            }
            wire::NodeProperty::PopupFor(id) => built.set_popup_for(accesskit::NodeId(id)),
            wire::NodeProperty::Label(value) => built.set_label(value),
            wire::NodeProperty::Description(value) => built.set_description(value),
            wire::NodeProperty::Value(value) => built.set_value(value),
            wire::NodeProperty::AccessKey(value) => built.set_access_key(value),
            wire::NodeProperty::AuthorId(value) => built.set_author_id(value),
            wire::NodeProperty::ClassName(value) => built.set_class_name(value),
            wire::NodeProperty::FontFamily(value) => built.set_font_family(value),
            wire::NodeProperty::HtmlTag(value) => built.set_html_tag(value),
            wire::NodeProperty::InnerHtml(value) => built.set_inner_html(value),
            wire::NodeProperty::KeyboardShortcut(value) => built.set_keyboard_shortcut(value),
            wire::NodeProperty::Language(value) => built.set_language(value),
            wire::NodeProperty::Placeholder(value) => built.set_placeholder(value),
            wire::NodeProperty::RoleDescription(value) => built.set_role_description(value),
            wire::NodeProperty::StateDescription(value) => built.set_state_description(value),
            wire::NodeProperty::Tooltip(value) => built.set_tooltip(value),
            wire::NodeProperty::Url(value) => built.set_url(value),
            wire::NodeProperty::RowIndexText(value) => built.set_row_index_text(value),
            wire::NodeProperty::ColumnIndexText(value) => built.set_column_index_text(value),
            wire::NodeProperty::BrailleLabel(value) => built.set_braille_label(value),
            wire::NodeProperty::BrailleRoleDescription(value) => {
                built.set_braille_role_description(value)
            }
            wire::NodeProperty::ScrollX(value) => built.set_scroll_x(value),
            wire::NodeProperty::ScrollXMin(value) => built.set_scroll_x_min(value),
            wire::NodeProperty::ScrollXMax(value) => built.set_scroll_x_max(value),
            wire::NodeProperty::ScrollY(value) => built.set_scroll_y(value),
            wire::NodeProperty::ScrollYMin(value) => built.set_scroll_y_min(value),
            wire::NodeProperty::ScrollYMax(value) => built.set_scroll_y_max(value),
            wire::NodeProperty::NumericValue(value) => built.set_numeric_value(value),
            wire::NodeProperty::MinNumericValue(value) => built.set_min_numeric_value(value),
            wire::NodeProperty::MaxNumericValue(value) => built.set_max_numeric_value(value),
            wire::NodeProperty::NumericValueStep(value) => built.set_numeric_value_step(value),
            wire::NodeProperty::NumericValueJump(value) => built.set_numeric_value_jump(value),
            wire::NodeProperty::FontSize(value) => built.set_font_size(value),
            wire::NodeProperty::FontWeight(value) => built.set_font_weight(value),
            wire::NodeProperty::Expanded(value) => built.set_expanded(value),
            wire::NodeProperty::Selected(value) => built.set_selected(value),
            wire::NodeProperty::RowCount(value) => built.set_row_count(value as usize),
            wire::NodeProperty::ColumnCount(value) => built.set_column_count(value as usize),
            wire::NodeProperty::RowIndex(value) => built.set_row_index(value as usize),
            wire::NodeProperty::ColumnIndex(value) => built.set_column_index(value as usize),
            wire::NodeProperty::RowSpan(value) => built.set_row_span(value as usize),
            wire::NodeProperty::ColumnSpan(value) => built.set_column_span(value as usize),
            wire::NodeProperty::Level(value) => built.set_level(value as usize),
            wire::NodeProperty::SizeOfSet(value) => built.set_size_of_set(value as usize),
            wire::NodeProperty::PositionInSet(value) => built.set_position_in_set(value as usize),
            wire::NodeProperty::ColorValue(value) => built.set_color_value(color_from(value)),
            wire::NodeProperty::BackgroundColor(value) => {
                built.set_background_color(color_from(value))
            }
            wire::NodeProperty::ForegroundColor(value) => {
                built.set_foreground_color(color_from(value))
            }
            wire::NodeProperty::Overline(value) => built.set_overline(accesskit::TextDecoration {
                style: text_decoration_style_from(value.style),
                color: color_from(value.color),
            }),
            wire::NodeProperty::Strikethrough(value) => {
                built.set_strikethrough(accesskit::TextDecoration {
                    style: text_decoration_style_from(value.style),
                    color: color_from(value.color),
                })
            }
            wire::NodeProperty::Underline(value) => {
                built.set_underline(accesskit::TextDecoration {
                    style: text_decoration_style_from(value.style),
                    color: color_from(value.color),
                })
            }
            wire::NodeProperty::CharacterLengths(value) => built.set_character_lengths(value),
            wire::NodeProperty::WordStarts(value) => built.set_word_starts(value),
            wire::NodeProperty::CharacterPositions(value) => built.set_character_positions(value),
            wire::NodeProperty::CharacterWidths(value) => built.set_character_widths(value),
            wire::NodeProperty::Invalid(value) => built.set_invalid(invalid_from(value)),
            wire::NodeProperty::Toggled(value) => built.set_toggled(toggled_from(value)),
            wire::NodeProperty::Live(value) => built.set_live(live_from(value)),
            wire::NodeProperty::TextDirection(value) => {
                built.set_text_direction(text_direction_from(value))
            }
            wire::NodeProperty::Orientation(value) => {
                built.set_orientation(orientation_from(value))
            }
            wire::NodeProperty::SortDirection(value) => {
                built.set_sort_direction(sort_direction_from(value))
            }
            wire::NodeProperty::AriaCurrent(value) => {
                built.set_aria_current(aria_current_from(value))
            }
            wire::NodeProperty::AutoComplete(value) => {
                built.set_auto_complete(auto_complete_from(value))
            }
            wire::NodeProperty::HasPopup(value) => built.set_has_popup(has_popup_from(value)),
            wire::NodeProperty::ListStyle(value) => built.set_list_style(list_style_from(value)),
            wire::NodeProperty::TextAlign(value) => built.set_text_align(text_align_from(value)),
            wire::NodeProperty::VerticalOffset(value) => {
                built.set_vertical_offset(vertical_offset_from(value))
            }
            wire::NodeProperty::Transform(coefficients) => {
                if let Ok(coefficients) = <[f64; 6]>::try_from(coefficients) {
                    built.set_transform(accesskit::Affine::new(coefficients));
                }
            }
            wire::NodeProperty::Bounds(bounds) => built.set_bounds(accesskit::Rect {
                x0: bounds.x0,
                y0: bounds.y0,
                x1: bounds.x1,
                y1: bounds.y1,
            }),
            wire::NodeProperty::TextSelection(selection) => {
                built.set_text_selection(accesskit::TextSelection {
                    anchor: position_from(selection.anchor),
                    focus: position_from(selection.focus),
                })
            }
            wire::NodeProperty::TreeId(tree) => {
                built.set_tree_id(accesskit::TreeId(uuid::Uuid::from_bytes(tree)))
            }
            wire::NodeProperty::CustomActions(actions) => built.set_custom_actions(
                actions
                    .into_iter()
                    .map(|action| accesskit::CustomAction {
                        id: action.id,
                        description: action.description.into(),
                    })
                    .collect::<Vec<_>>(),
            ),
        }
    }
    (accesskit::NodeId(node.id), built)
}

fn color_to(color: accesskit::Color) -> wire::Color {
    wire::Color {
        red: color.red,
        green: color.green,
        blue: color.blue,
        alpha: color.alpha,
    }
}

fn color_from(color: wire::Color) -> accesskit::Color {
    accesskit::Color {
        red: color.red,
        green: color.green,
        blue: color.blue,
        alpha: color.alpha,
    }
}

fn tree_update_to(update: &accesskit::TreeUpdate) -> wire::TreeUpdate {
    wire::TreeUpdate {
        nodes: update
            .nodes
            .iter()
            .map(|(id, node)| node_to(*id, node))
            .collect(),
        tree: update.tree.as_ref().map(|tree| wire::Tree {
            root: tree.root.0,
            toolkit_name: tree.toolkit_name.clone(),
            toolkit_version: tree.toolkit_version.clone(),
        }),
        tree_id: update.tree_id.0.into_bytes(),
        focus: update.focus.0,
    }
}

fn tree_update_from(update: wire::TreeUpdate) -> accesskit::TreeUpdate {
    accesskit::TreeUpdate {
        nodes: update.nodes.into_iter().map(node_from).collect(),
        tree: update.tree.map(|tree| accesskit::Tree {
            root: accesskit::NodeId(tree.root),
            toolkit_name: tree.toolkit_name,
            toolkit_version: tree.toolkit_version,
        }),
        tree_id: accesskit::TreeId(uuid::Uuid::from_bytes(update.tree_id)),
        focus: accesskit::NodeId(update.focus),
    }
}

impl From<&accesskit::TreeUpdate> for wire::TreeUpdate {
    fn from(update: &accesskit::TreeUpdate) -> Self {
        tree_update_to(update)
    }
}

impl From<wire::TreeUpdate> for accesskit::TreeUpdate {
    fn from(update: wire::TreeUpdate) -> Self {
        tree_update_from(update)
    }
}

impl From<crate::Description> for wire::Description {
    fn from(description: crate::Description) -> Self {
        Self {
            tree: description.tree.as_ref().map(tree_update_to),
            actions: description.actions,
        }
    }
}

impl From<wire::Description> for crate::Description {
    fn from(description: wire::Description) -> Self {
        Self {
            tree: description.tree.map(tree_update_from),
            actions: description.actions,
        }
    }
}
