//! Interactive viewer powered by `winit` + `softbuffer`.
//!
//! The document is rendered to an RGBA pixmap on demand and blitted into the
//! software framebuffer. Navigation, zoom, rotation, fit-to-width and text
//! search are all keyboard driven. No widget toolkit is involved.

use std::num::NonZeroU32;
use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use mupdf::{Matrix, Quad};

use crate::document::ViewerDocument;
use crate::error::Result;

const MIN_SCALE: f32 = 0.25;
const MAX_SCALE: f32 = 8.0;

/// Run the interactive viewer for `path`.
pub fn run(path: &str) -> Result<()> {
    let doc = ViewerDocument::open(path)?;
    let event_loop = EventLoop::new()
        .map_err(|e| crate::error::Error::Io(std::io::Error::other(format!("{e}"))))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = ViewerApp {
        doc,
        path: path.to_string(),
        window: None,
        context: None,
        surface: None,
        scale: 1.0,
        rotation: 0u8,
        page: 0,
        fit_width: false,
        search_editing: false,
        search_input: String::new(),
        hits: Vec::new(),
        hit_index: 0,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| crate::error::Error::Io(std::io::Error::other(format!("{e}"))))?;
    Ok(())
}

struct ViewerApp {
    doc: ViewerDocument,
    path: String,
    window: Option<Rc<Window>>,
    /// `softbuffer::Surface` does not borrow `Context` for its lifetime (the
    /// `&Context` in `Surface::new` is only used transiently), so both can be
    /// owned in the same struct.
    context: Option<softbuffer::Context<Rc<Window>>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    scale: f32,
    rotation: u8,
    page: usize,
    fit_width: bool,
    /// ASCII line-edit search box active state.
    search_editing: bool,
    search_input: String,
    /// Current page's search hit quads (PDF coordinates).
    hits: Vec<Quad>,
    hit_index: usize,
}

impl ViewerApp {
    /// Render the current page at the current scale/rotation and blit it,
    /// centered, into the window framebuffer.
    fn present(&mut self) {
        let (window, surface) = match (self.window.as_ref(), self.surface.as_mut()) {
            (Some(w), Some(s)) => (w, s),
            _ => return,
        };
        let size = window.inner_size();
        let rendered = match self.doc.render(self.page, self.scale, self.rotation) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("render error: {e}");
                return;
            }
        };
        let sw = match NonZeroU32::new(size.width) {
            Some(v) => v,
            None => return,
        };
        let sh = match NonZeroU32::new(size.height) {
            Some(v) => v,
            None => return,
        };
        if let Err(e) = surface.resize(sw, sh) {
            eprintln!("resize error: {e}");
            return;
        }
        let mut buffer = match surface.buffer_mut() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("buffer error: {e}");
                return;
            }
        };
        let ctm = Self::ctm_for(self.scale, self.rotation);
        crate::render::blit_to_buffer(
            &rendered,
            &mut buffer,
            size.width as usize,
            size.height as usize,
            &ctm,
            &self.hits,
            self.hit_index,
        );
        if let Err(e) = buffer.present() {
            eprintln!("present error: {e}");
        }
    }

    /// Build the render/transform matrix matching `ViewerDocument::render`.
    fn ctm_for(scale: f32, rotation: u8) -> Matrix {
        crate::render::ctm_for(scale, rotation)
    }

    /// Re-run the search for the current page with the given term.
    fn run_search(&mut self, term: &str) {
        if term.is_empty() {
            self.hits.clear();
            self.hit_index = 0;
            return;
        }
        match self.doc.search(self.page, term) {
            Ok(hits) => {
                self.hits = hits;
                self.hit_index = 0;
            }
            Err(e) => eprintln!("search error: {e}"),
        }
    }

    fn recompute_scale_to_fit(&mut self, win_w: u32) {
        if !self.fit_width {
            return;
        }
        if let Ok((pw, _ph)) = self.doc.page_size_pt(self.page) {
            if pw > 0.0 {
                let fit = win_w as f32 / pw;
                self.scale = fit.clamp(MIN_SCALE, MAX_SCALE);
            }
        }
    }

    fn status(&self) -> String {
        let fit = if self.fit_width {
            "  ·  fit-width"
        } else {
            ""
        };
        let base = format!(
            "{}  ·  page {}/{}  ·  {:.0}%  ·  rot {}°{}",
            self.path,
            self.page + 1,
            self.doc.page_count(),
            self.scale * 100.0,
            self.rotation,
            fit,
        );
        if self.search_editing {
            format!("{base}  ·  /{}", self.search_input)
        } else if !self.hits.is_empty() {
            format!("{base}  ·  {}/{} hits", self.hit_index + 1, self.hits.len())
        } else {
            base
        }
    }
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // Handle redundant resume events gracefully.
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("MuPDF mini")
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 1200.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("window build error: {e}");
                return;
            }
        };
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("softbuffer context error: {e}");
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("softbuffer surface error: {e}");
                return;
            }
        };
        window.set_visible(true);
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let window = match self.window.clone() {
            Some(w) => w,
            None => return,
        };
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.recompute_scale_to_fit(window.inner_size().width);
                self.present();
                eprint!("\r{}", self.status());
            }
            WindowEvent::Resized(_) => {
                window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Search line-edit mode intercepts most keys.
                if self.search_editing {
                    self.handle_search_key(event_loop, &event.logical_key, &window);
                    return;
                }
                use winit::keyboard::{Key, NamedKey};
                match &event.logical_key {
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.page + 1 < self.doc.page_count() {
                            self.page += 1;
                            window.request_redraw();
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.page > 0 {
                            self.page -= 1;
                            window.request_redraw();
                        }
                    }
                    Key::Character(c) if c.as_str() == "l" => {
                        if self.page + 1 < self.doc.page_count() {
                            self.page += 1;
                            window.request_redraw();
                        }
                    }
                    Key::Character(c) if c.as_str() == "h" => {
                        if self.page > 0 {
                            self.page -= 1;
                            window.request_redraw();
                        }
                    }
                    Key::Character(c) if c.as_str() == "+" || c.as_str() == "=" => {
                        self.fit_width = false;
                        self.scale = (self.scale * 1.2).clamp(MIN_SCALE, MAX_SCALE);
                        window.request_redraw();
                    }
                    Key::Character(c) if c.as_str() == "-" => {
                        self.fit_width = false;
                        self.scale = (self.scale / 1.2).clamp(MIN_SCALE, MAX_SCALE);
                        window.request_redraw();
                    }
                    Key::Character(c) if c.as_str() == "0" => {
                        self.fit_width = false;
                        self.scale = 1.0;
                        window.request_redraw();
                    }
                    Key::Character(c) if c.as_str() == "w" => {
                        self.fit_width = true;
                        self.recompute_scale_to_fit(window.inner_size().width);
                        window.request_redraw();
                    }
                    Key::Character(c) if c.as_str() == "r" => {
                        self.rotation = ((self.rotation as u16 + 90) % 360) as u8;
                        window.request_redraw();
                    }
                    Key::Character(c) if c.as_str() == "/" => {
                        self.search_editing = true;
                        self.search_input.clear();
                        window.request_redraw();
                    }
                    Key::Character(c) if c.as_str() == "n" => {
                        if !self.hits.is_empty() {
                            self.hit_index = (self.hit_index + 1) % self.hits.len();
                            window.request_redraw();
                        }
                    }
                    Key::Character(c) if c.as_str() == "N" => {
                        if !self.hits.is_empty() {
                            self.hit_index =
                                (self.hit_index + self.hits.len() - 1) % self.hits.len();
                            window.request_redraw();
                        }
                    }
                    Key::Character(c) if c.as_str() == "q" => {
                        event_loop.exit();
                    }
                    Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dir = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => (p.y / 100.0) as f32,
                };
                self.fit_width = false;
                if dir > 0.0 {
                    self.scale = (self.scale * 1.1).clamp(MIN_SCALE, MAX_SCALE);
                } else if dir < 0.0 {
                    self.scale = (self.scale / 1.1).clamp(MIN_SCALE, MAX_SCALE);
                }
                window.request_redraw();
            }
            _ => {}
        }
    }
}

impl ViewerApp {
    /// Handle keys while the search box is active (ASCII line editing).
    fn handle_search_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        key: &winit::keyboard::Key,
        window: &Rc<Window>,
    ) {
        use winit::keyboard::{Key, NamedKey};
        match key {
            Key::Named(NamedKey::Escape) => {
                self.search_editing = false;
                self.search_input.clear();
                window.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                let term = self.search_input.clone();
                self.search_editing = false;
                self.run_search(&term);
                window.request_redraw();
            }
            Key::Named(NamedKey::Backspace) => {
                self.search_input.pop();
                window.request_redraw();
            }
            Key::Character(c) => {
                // Accept printable ASCII only (no IME); reject control chars.
                for ch in c.chars() {
                    if !ch.is_control() {
                        self.search_input.push(ch);
                    }
                }
                window.request_redraw();
            }
            _ => {}
        }
        let _ = event_loop;
    }
}
