// freetype-sys's build script, compiling FreeType with libpng, and libpng and
// zlib from the sources the fixup names, so that FreeType loads the colour
// glyphs that emoji fonts keep as PNG images. It always compiles the FreeType
// it vendors rather than asking pkg-config for one. zlib's names are prefixed
// so that they cannot meet a system zlib's.
use std::env;
use std::fs;
use std::path::PathBuf;

fn add_sources(build: &mut cc::Build, root: &str, files: &[&str]) {
    let root = std::path::Path::new(root);
    build.files(files.iter().map(|src| {
        let mut p = root.join(src);
        p.set_extension("c");
        p
    }));

    build.include(root);
}

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let zlib = PathBuf::from(env::var("ZLIB_SOURCE").unwrap());
    let libpng = PathBuf::from(env::var("LIBPNG_SOURCE").unwrap());
    let include = out.join("include");
    fs::create_dir_all(&include).unwrap();
    fs::copy(
        libpng.join("scripts/pnglibconf.h.prebuilt"),
        include.join("pnglibconf.h"),
    )
    .unwrap();

    let mut build = cc::Build::new();

    build
        .cpp(true)
        .warnings(false)
        .include(".")
        .include("freetype2/include")
        .define("FT2_BUILD_LIBRARY", None)
        .define("FT_CONFIG_OPTION_USE_PNG", None)
        .include(&libpng)
        .include(&include);

    add_sources(
        &mut build,
        "freetype2/src",
        &[
            "autofit/autofit",
            "base/ftbase",
            "base/ftbbox",
            "base/ftbdf",
            "base/ftbitmap",
            "base/ftcid",
            "base/ftdebug",
            "base/ftfstype",
            "base/ftgasp",
            "base/ftglyph",
            "base/ftgxval",
            "base/ftinit",
            "base/ftmm",
            "base/ftotval",
            "base/ftpatent",
            "base/ftpfr",
            "base/ftstroke",
            "base/ftsynth",
            "base/ftsystem",
            "base/fttype1",
            "base/ftwinfnt",
            "bdf/bdf",
            "bzip2/ftbzip2",
            "cache/ftcache",
            "cff/cff",
            "cid/type1cid",
            "gzip/ftgzip",
            "lzw/ftlzw",
            "pcf/pcf",
            "pfr/pfr",
            "psaux/psaux",
            "pshinter/pshinter",
            "psnames/psnames",
            "raster/raster",
            "sdf/sdf",
            "svg/svg",
            "sfnt/sfnt",
            "smooth/smooth",
            "truetype/truetype",
            "type1/type1",
            "type42/type42",
            "winfonts/winfnt",
        ],
    );

    build.compile("freetype2");

    let mut png = cc::Build::new();
    png.warnings(false)
        .include(&libpng)
        .include(&include)
        .include(&zlib)
        .define("Z_PREFIX", None);
    for optimisation in [
        "PNG_ARM_NEON_OPT",
        "PNG_INTEL_SSE_OPT",
        "PNG_MIPS_MSA_OPT",
        "PNG_MIPS_MMI_OPT",
        "PNG_POWERPC_VSX_OPT",
        "PNG_LOONGARCH_LSX_OPT",
        "PNG_RISCV_RVV_OPT",
    ] {
        png.define(optimisation, "0");
    }
    for file in [
        "png", "pngerror", "pngget", "pngmem", "pngpread", "pngread", "pngrio", "pngrtran",
        "pngrutil", "pngset", "pngtrans", "pngwio", "pngwrite", "pngwtran", "pngwutil",
    ] {
        png.file(libpng.join(file).with_extension("c"));
    }
    png.compile("freetype_png");

    let mut z = cc::Build::new();
    z.warnings(false).include(&zlib).define("Z_PREFIX", None);
    for file in [
        "adler32", "compress", "crc32", "deflate", "infback", "inffast", "inflate", "inftrees",
        "trees", "uncompr", "zutil",
    ] {
        z.file(zlib.join(file).with_extension("c"));
    }
    z.compile("freetype_zlib");
}
