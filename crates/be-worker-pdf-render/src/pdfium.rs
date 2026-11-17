//! PDFium, which renders the worker's pages, through its C interface.

use std::{
    ffi::{c_int, c_ulong, c_void},
    ptr,
    sync::Once,
    time::Instant,
};

use be_lib_geometry::{Pos2, Rect, Vec2, vec2};

use be_worker_pdf::{DETAIL_MAX_DIM, MIN_SCALE, Measure, Rendered, Target};

const MAX_PAGE_DIM: f32 = 100_000.0;
const WHITE: c_ulong = 0xFFFF_FFFF;
const FPDF_ANNOT: c_int = 0x01;
const FPDF_REVERSE_BYTE_ORDER: c_int = 0x10;

// unsafe(ffi)
unsafe extern "C" {
    fn FPDF_InitLibrary();
    fn FPDF_SetSystemFontInfo(font_info: *mut c_void);
    fn FPDF_LoadMemDocument(data: *const c_void, size: c_int, password: *const u8) -> *mut c_void;
    fn FPDF_GetLastError() -> c_ulong;
    fn FPDF_CloseDocument(document: *mut c_void);
    fn FPDF_GetPageCount(document: *mut c_void) -> c_int;
    fn FPDF_LoadPage(document: *mut c_void, index: c_int) -> *mut c_void;
    fn FPDF_ClosePage(page: *mut c_void);
    fn FPDF_GetPageWidthF(page: *mut c_void) -> f32;
    fn FPDF_GetPageHeightF(page: *mut c_void) -> f32;
    fn FPDF_RenderPageBitmap(
        bitmap: *mut c_void,
        page: *mut c_void,
        start_x: c_int,
        start_y: c_int,
        size_x: c_int,
        size_y: c_int,
        rotate: c_int,
        flags: c_int,
    );
    fn FPDFBitmap_Create(width: c_int, height: c_int, alpha: c_int) -> *mut c_void;
    fn FPDFBitmap_FillRect(
        bitmap: *mut c_void,
        left: c_int,
        top: c_int,
        width: c_int,
        height: c_int,
        color: c_ulong,
    ) -> c_int;
    fn FPDFBitmap_GetBuffer(bitmap: *mut c_void) -> *mut u8;
    fn FPDFBitmap_GetStride(bitmap: *mut c_void) -> c_int;
    fn FPDFBitmap_Destroy(bitmap: *mut c_void);
}

/// Starts PDFium as the worker starts: once per instance, which is also once for every worker
/// an app's tests serve in their own (be-sdk-test's `serve_worker`).
// unsafe(ffi)
pub(crate) fn start() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| unsafe {
        FPDF_InitLibrary();
        FPDF_SetSystemFontInfo(ptr::null_mut());
    });
}

fn record(measures: &mut Vec<Measure>, name: &str, since: Instant) {
    measures.push(Measure::Duration(name.to_owned(), since.elapsed()));
}

fn count(measures: &mut Vec<Measure>, name: &str, value: u64) {
    measures.push(Measure::Count(name.to_owned(), value));
}

struct Document(*mut c_void);

// unsafe(ffi)
impl Document {
    fn load(data: &[u8]) -> Result<Self, String> {
        let size = c_int::try_from(data.len()).map_err(|_| "This PDF is too large".to_owned())?;
        let document = unsafe { FPDF_LoadMemDocument(data.as_ptr().cast(), size, ptr::null()) };
        if document.is_null() {
            return Err(load_error(unsafe { FPDF_GetLastError() }));
        }
        Ok(Self(document))
    }

    fn page_count(&self) -> usize {
        unsafe { FPDF_GetPageCount(self.0) }.max(0) as usize
    }

    fn page(&self, index: usize) -> Result<Page<'_>, String> {
        let page = unsafe { FPDF_LoadPage(self.0, index as c_int) };
        match page.is_null() {
            true => Err(format!("Could not load page {}", index + 1)),
            false => Ok(Page(page, std::marker::PhantomData)),
        }
    }
}

// unsafe(ffi)
impl Drop for Document {
    fn drop(&mut self) {
        unsafe { FPDF_CloseDocument(self.0) }
    }
}

struct Page<'document>(*mut c_void, std::marker::PhantomData<&'document Document>);

// unsafe(ffi)
impl Page<'_> {
    fn size(&self) -> Vec2 {
        unsafe { vec2(FPDF_GetPageWidthF(self.0), FPDF_GetPageHeightF(self.0)) }
    }
}

// unsafe(ffi)
impl Drop for Page<'_> {
    fn drop(&mut self) {
        unsafe { FPDF_ClosePage(self.0) }
    }
}

struct Bitmap(*mut c_void);

// unsafe(ffi)
impl Bitmap {
    fn new(width: c_int, height: c_int) -> Result<Self, String> {
        let bitmap = unsafe { FPDFBitmap_Create(width, height, 1) };
        if bitmap.is_null() {
            return Err(format!("Could not allocate a {width} by {height} bitmap"));
        }
        unsafe { FPDFBitmap_FillRect(bitmap, 0, 0, width, height, WHITE) };
        Ok(Self(bitmap))
    }

    fn rgba(&self, width: usize, height: usize) -> Vec<u8> {
        let stride = unsafe { FPDFBitmap_GetStride(self.0) }.max(0) as usize;
        let buffer = unsafe { FPDFBitmap_GetBuffer(self.0) };
        let pixels = unsafe { std::slice::from_raw_parts(buffer, stride * height) };
        let mut rgba = Vec::with_capacity(width * height * 4);
        for row in pixels.chunks_exact(stride) {
            rgba.extend_from_slice(&row[..width * 4]);
        }
        rgba
    }
}

// unsafe(ffi)
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe { FPDFBitmap_Destroy(self.0) }
    }
}

fn load_error(code: c_ulong) -> String {
    match code {
        2 => "The PDF file could not be found".to_owned(),
        3 => "This is not a PDF file, or it is damaged".to_owned(),
        4 => "This PDF is protected by a password".to_owned(),
        5 => "This PDF uses an unsupported security scheme".to_owned(),
        6 => "This PDF has a page that could not be read".to_owned(),
        code => format!("PDFium could not open this PDF (error {code})"),
    }
}

/// Renders `target` of page `page` of the PDF `data`, adding what it measured to `measures`.
// unsafe(ffi)
pub(crate) fn render(
    data: &[u8],
    page: usize,
    target: Target,
    measures: &mut Vec<Measure>,
) -> Result<Rendered, String> {
    let started = Instant::now();
    let rendered = render_tile(data, page, target, measures);
    record(measures, "Worker total", started);
    rendered
}

// unsafe(ffi)
fn render_tile(
    data: &[u8],
    page: usize,
    target: Target,
    measures: &mut Vec<Measure>,
) -> Result<Rendered, String> {
    let phase = Instant::now();
    let document = Document::load(data)?;
    record(measures, "Document load", phase);

    let phase = Instant::now();
    let page_count = document.page_count();
    if page_count == 0 {
        return Err("This PDF has no pages".into());
    }
    let index = page.min(page_count - 1);
    let pdf_page = document.page(index)?;
    let page_size_pts = pdf_page.size();
    record(measures, "Page setup", phase);

    if page_size_pts.x <= 0.0 || page_size_pts.y <= 0.0 {
        return Err("This PDF page is empty".into());
    }
    let bounds = Rect::from_min_size(Pos2::ZERO, page_size_pts);
    let (requested_scale, region) = match target {
        Target::FullPage {
            max_width,
            max_height,
        } => (
            (max_width.max(1.0) / page_size_pts.x).min(max_height.max(1.0) / page_size_pts.y),
            bounds,
        ),
        Target::ShownView {
            scale,
            origin_pts,
            size_pts,
        } => (
            scale,
            Rect::from_min_size(origin_pts, size_pts).intersect(bounds),
        ),
    };
    let scale = requested_scale.clamp(MIN_SCALE, MAX_PAGE_DIM / page_size_pts.longest_side());

    let page_width = ((page_size_pts.x * scale).round() as c_int).max(1);
    let page_height = ((page_size_pts.y * scale).round() as c_int).max(1);
    let origin_px = vec2(
        (region.min.x * scale).round(),
        (region.min.y * scale).round(),
    );
    let width = ((region.width() * scale).round() as c_int).clamp(1, DETAIL_MAX_DIM as c_int);
    let height = ((region.height() * scale).round() as c_int).clamp(1, DETAIL_MAX_DIM as c_int);
    count(measures, "Output width", width as u64);
    count(measures, "Output height", height as u64);

    let phase = Instant::now();
    let bitmap = Bitmap::new(width, height)?;
    record(measures, "Bitmap allocation", phase);
    let phase = Instant::now();
    unsafe {
        FPDF_RenderPageBitmap(
            bitmap.0,
            pdf_page.0,
            -origin_px.x as c_int,
            -origin_px.y as c_int,
            page_width,
            page_height,
            0,
            FPDF_ANNOT | FPDF_REVERSE_BYTE_ORDER,
        );
    }
    record(measures, "PDFium render", phase);
    let phase = Instant::now();
    let rgba = bitmap.rgba(width as usize, height as usize);
    record(measures, "RGBA copy", phase);
    count(measures, "Pixels", width as u64 * height as u64);
    Ok(Rendered {
        page_count,
        page_index: index,
        page_size_pts,
        scale,
        origin_pts: Pos2::new(origin_px.x / scale, origin_px.y / scale),
        size_pts: vec2(width as f32 / scale, height as f32 / scale),
        width: width as u32,
        height: height as u32,
        rgba,
    })
}
