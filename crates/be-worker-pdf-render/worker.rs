//! The PDF renderer's module: its entry point, which buck links as :module (`worker()`).

be_sdk_worker::worker!(be_worker_pdf_render::Renderer);
