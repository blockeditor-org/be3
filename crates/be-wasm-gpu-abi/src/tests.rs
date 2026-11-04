use super::*;

macro_rules! listed {
    ($($glue:path, $module:literal, $name:literal, $export:literal, ($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        &[$(($name, stringify!($($kind),*), stringify!($($result)?))),*]
    };
}

fn imports() -> &'static [(&'static str, &'static str, &'static str)] {
    gpu_local_imports!(listed)
}

mod a_descriptor_round_trips_through_its_encoding;
mod every_resource_has_a_drop;
mod the_calls_made_per_draw_take_only_wasm_values;
