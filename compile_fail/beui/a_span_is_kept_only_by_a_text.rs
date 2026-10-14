use beui::reactive::{Children, Span};

fn case(_spans: Children<Span>) {} //~ ERROR is not something a parent can keep a run of
