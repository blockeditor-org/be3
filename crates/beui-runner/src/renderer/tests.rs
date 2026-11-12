use super::*;

mod a_leased_renderer_stops_drawing_once_its_surface_is_revoked;

struct Window;

impl raw_window_handle::HasWindowHandle for Window {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::Unavailable)
    }
}

impl raw_window_handle::HasDisplayHandle for Window {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Err(raw_window_handle::HandleError::Unavailable)
    }
}

/// Counts what reaches the surface.
#[derive(Clone, Default)]
struct Touches(std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>);

impl Touches {
    fn take(&self) -> Vec<&'static str> {
        self.0.take()
    }
}

struct Counting(Touches);

impl Renderer for Counting {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn attach(
        &mut self,
        _window: Arc<dyn WindowHandle>,
        _width: u32,
        _height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.0.0.borrow_mut().push("attach");
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        self.0.0.borrow_mut().push("resize");
    }

    fn physical(&self) -> Option<Vec2> {
        Some(Vec2::new(10.0, 10.0))
    }

    fn prepare(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        self.0.0.borrow_mut().push("prepare");
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        self.0.0.borrow_mut().push("present");
        false
    }
}
