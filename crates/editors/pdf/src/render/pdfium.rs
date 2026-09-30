use std::{
    ffi::{c_int, c_ulong, c_void},
    ptr,
    sync::{OnceLock, mpsc},
    thread,
    time::Instant,
};

use block_editor_beui::{
    PerformanceReporter, Waker,
    beui::{Image, Pos2, Rect, vec2},
};

use super::{
    DETAIL_MAX_DIM, MIN_SCALE, RenderJob, RenderJobMessage, RenderJobResult, RenderTarget,
    RenderedTile,
};

const MAX_PAGE_DIM: f32 = 100_000.0;
const WHITE: c_ulong = 0xFFFF_FFFF;
const FPDF_ANNOT: c_int = 0x01;
const FPDF_REVERSE_BYTE_ORDER: c_int = 0x10;

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

struct Job {
    data: Vec<u8>,
    page: usize,
    target: RenderTarget,
    requested_at: Instant,
    sender: mpsc::Sender<RenderJobMessage>,
    waker: Waker,
    performance: PerformanceReporter,
}

pub(crate) fn spawn_render_job(
    data: Vec<u8>,
    page: usize,
    target: RenderTarget,
    waker: Waker,
    performance: PerformanceReporter,
) -> RenderJob {
    let (sender, receiver) = mpsc::channel();
    let job = Job {
        data,
        page,
        target,
        requested_at: Instant::now(),
        sender,
        waker,
        performance,
    };
    if let Err(mpsc::SendError(job)) = worker().send(job) {
        let _ = job.sender.send(RenderJobMessage {
            completed_at: Instant::now(),
            result: Err("The PDF renderer stopped.".to_owned()),
        });
    }
    RenderJob { receiver }
}

fn worker() -> &'static mpsc::Sender<Job> {
    static WORKER: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (sender, jobs) = mpsc::channel::<Job>();
        let _ = thread::Builder::new()
            .name("pdf-render".into())
            .spawn(move || {
                unsafe {
                    FPDF_InitLibrary();
                    FPDF_SetSystemFontInfo(ptr::null_mut());
                }
                for job in jobs {
                    job.performance
                        .record_duration("Queued", job.requested_at.elapsed());
                    let started = Instant::now();
                    let result = render_tile(&job.data, job.page, job.target, &job.performance);
                    let completed_at = Instant::now();
                    job.performance
                        .record_duration("Worker total", completed_at.duration_since(started));
                    let _ = job.sender.send(RenderJobMessage {
                        completed_at,
                        result,
                    });
                    job.waker.wake();
                }
            });
        sender
    })
}

struct Document(*mut c_void);

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

impl Drop for Document {
    fn drop(&mut self) {
        unsafe { FPDF_CloseDocument(self.0) }
    }
}

struct Page<'document>(*mut c_void, std::marker::PhantomData<&'document Document>);

impl Page<'_> {
    fn size(&self) -> block_editor_beui::beui::Vec2 {
        unsafe { vec2(FPDF_GetPageWidthF(self.0), FPDF_GetPageHeightF(self.0)) }
    }
}

impl Drop for Page<'_> {
    fn drop(&mut self) {
        unsafe { FPDF_ClosePage(self.0) }
    }
}

struct Bitmap(*mut c_void);

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

fn render_tile(
    data: &[u8],
    page: usize,
    target: RenderTarget,
    performance: &PerformanceReporter,
) -> RenderJobResult {
    let phase = Instant::now();
    let document = Document::load(data)?;
    performance.record_duration("Document load", phase.elapsed());

    let phase = Instant::now();
    let page_count = document.page_count();
    if page_count == 0 {
        return Err("This PDF has no pages".into());
    }
    let index = page.min(page_count - 1);
    let pdf_page = document.page(index)?;
    let page_size_pts = pdf_page.size();
    performance.record_duration("Page setup", phase.elapsed());

    if page_size_pts.x <= 0.0 || page_size_pts.y <= 0.0 {
        return Err("This PDF page is empty".into());
    }
    let bounds = Rect::from_min_size(Pos2::ZERO, page_size_pts);
    let (requested_scale, region) = match target {
        RenderTarget::FullPage {
            max_width,
            max_height,
        } => (
            (max_width.max(1.0) / page_size_pts.x).min(max_height.max(1.0) / page_size_pts.y),
            bounds,
        ),
        RenderTarget::Region {
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
    performance.record_count("Output width", width as u64);
    performance.record_count("Output height", height as u64);

    let phase = Instant::now();
    let bitmap = Bitmap::new(width, height)?;
    performance.record_duration("Bitmap allocation", phase.elapsed());
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
    performance.record_duration("PDFium render", phase.elapsed());
    let phase = Instant::now();
    let image = Image::from_rgba(
        width as u32,
        height as u32,
        bitmap.rgba(width as usize, height as usize),
    );
    performance.record_duration("RGBA copy", phase.elapsed());
    performance.record_count("Pixels", width as u64 * height as u64);
    Ok(RenderedTile {
        page_count,
        page_index: index,
        page_size_pts,
        scale,
        origin_pts: Pos2::new(origin_px.x / scale, origin_px.y / scale),
        size_pts: vec2(width as f32 / scale, height as f32 / scale),
        image,
    })
}
