use beui::primitives::Span;
use beui::reactive::Children;

fn case(_spans: Children<Span>) {} //~ ERROR is not something a parent can keep a run of
