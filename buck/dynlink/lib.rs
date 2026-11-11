// The root of every native dynamic library (defs.bzl): it holds nothing itself.
// The crates a library holds are its dependencies, which rustc links into it
// because they are loaded with --extern force:.
