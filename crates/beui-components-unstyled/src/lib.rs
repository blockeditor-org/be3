extern crate beui_view as beui;

pub mod button;
pub mod calendar;
pub mod choice;
pub mod color_area;
pub mod container;
pub mod context_menu;
pub mod date_time_field;
pub mod datetime;
pub mod disclosure;
pub mod dock;
pub mod drag;
pub mod fling;
pub mod floating;
pub mod menu;
pub mod menu_button;
pub mod pan_zoom;
pub mod pointer_lock;
pub mod popover;
pub mod pressable;
pub mod rubber_band;
pub mod scroll;
pub mod scrollbar;
pub mod select;
pub mod selectable;
pub mod slider;
pub mod stack;
pub mod text_area;
pub mod text_input;
pub mod time_list;
pub mod toggle;
pub mod tooltip;
pub mod tree;
pub mod typeahead;

pub use beui_core::drag_board::DragPoint;
pub use button::{Button, ButtonHandle, button_active, button_focused};
pub use calendar::{
    Calendar, CalendarDayHandle, CalendarHeaderHandle, CalendarMode, CalendarMonthHandle,
    CalendarYearHandle, calendar_active, calendar_mode, calendar_selected,
};
pub use choice::{Choice, ChoiceKind, ChoiceOption, ChoiceOptionHandle, choice_selected};
pub use color_area::{ColorArea, ColorAreaHandle, color_area_value};
pub use container::{Container, ContainerSize, container_size, narrower_than, shorter_than};
pub use context_menu::{ContextMenu, context_menu_menu, context_menu_overlay};
pub use date_time_field::{
    DateDraft, DateSegment, DateSegmentHandle, DateTimeField, DateTimeParts, date_time_field_text,
    date_time_field_value,
};
pub use disclosure::{Disclosure, DisclosureHandle, disclosure_open};
pub use dock::{
    Dock, DockDragged, DockDrop, DockGripHandle, DockLayout, DockMode, DockMores, DockPanelHandle,
    DockPreviewHandle, DockSplitter, DockSplitterHandle, DockStackHandle, DockState, DockTabHandle,
    DockTabMore, DockTree, DockTreeEntry, DockWindowHandle, Entry, GroupId, LeafId,
    MIN_PANE_LENGTH, MIN_SIDEBAR_WIDTH, SIDEBAR_WIDTH, SPLITTER_THICKNESS, Side, SplitId,
    SurfaceId, TabId, TabPosition, Tree, dock_actions, dock_more, dock_state, layout_surface,
    layout_tree, sidebar_size,
};
pub use drag::{
    DRAG_PREVIEW_OFFSET, DRAG_THRESHOLD, DragHandle, Draggable, DropHandle, DropTarget,
};
pub use floating::{Edge, Floating};
pub use menu::{
    MenuItem, MenuRowHandle, menu_list_len, menu_list_root_focusable, menu_list_row_button,
    menu_list_row_submenu_content,
};
pub use menu_button::{MenuButton, MenuButtonHandle};
pub use pan_zoom::{MAX_SCALE, MIN_SCALE, PanZoom, PanZoomHandle, PanZoomView, pan_zoom_view};
pub use pointer_lock::{PointerLock, PointerLockHandle};
pub use popover::{
    Popover, PopoverHandle, PopoverPlacement, PopoverTriggerHandle, popover_open, popover_trigger,
};
pub use pressable::Pressable;
pub use scroll::{Scroll, ScrollHandle, ScrollbarStyle, scroll_animating};
pub use scrollbar::{Scrollbar, ScrollbarHandle, thumb_length, thumb_start};
pub use select::{
    Select, SelectOptionHandle, SelectTriggerHandle, select_highlighted, select_open,
    select_option_button, select_search, select_selected, select_trigger,
};
pub use selectable::{Selectable, SelectableState, copy_selection, select_all, selectable_text};
pub use slider::{Slider, SliderHandle, SliderScale, slider_value};
pub use stack::Stack;
pub use text_area::text_area_handles;
pub use text_area::{
    Completer, Completion, CompletionMenu, RemoteTextCursor, SyntaxColors, TextArea,
    TextAreaColors, TextAreaLayout, TextAreaState, TextWidget, text_area_index_at, text_area_shown,
    text_area_state,
};
pub use text_input::text_input_handles;
pub use text_input::{
    TextInput, TextInputHandle, TextInputMenu, text_input_caret, text_input_focused,
    text_input_index_at, text_input_menu_row, text_input_selection, text_input_shown,
    text_input_text, text_input_value,
};
pub use time_list::{TimeList, TimeOptionHandle, time_list_selected};
pub use toggle::{Toggle, ToggleHandle, toggle_checked};
pub use tooltip::{TOOLTIP_DELAY, Tooltip, TooltipHandle};
pub use tree::{Tree, TreeItem, TreeRowHandle, tree_focused, tree_row_node};
