// The bindings third-party/rust/overlays/v8/build.rs copies into OUT_DIR, in
// place of the absolute path cargo's own build script hands the crate.
#![allow(unused)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]
include!(concat!(env!("OUT_DIR"), "/src_binding.rs"));
