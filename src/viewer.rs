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
use crate::render::RenderedPage;

const MIN_SCALE: f32 = 0.25;
const MAX_SCALE: f32 = 8.0;

/// Run the interactive viewer for `path`.
pub fn run(path: &str) -> Result<()> {
    let doc = ViewerDocument::open(path)?;
    let event_loop = EventLoop::new().map_err(|e| {
        crate::error::Error::Io(std::io::Error::other(format!("{e}")))
    })?;
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
    event_loop.run_app(&mut app).map_err(|e| {
        crate::error::Error::Io(std::io::Error::other(format!("{e}")))
    })?;
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
        blit(
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
        let mut ctm = Matrix::new_scale(scale, scale);
        if rotation != 0 {
            ctm.concat(Matrix::new_rotate(rotation as f32));
        }
        ctm
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
        let fit = if self.fit_width { "  ·  fit-width" } else { "" };
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

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed =>
            {
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
                    Key::Character(c)
                        if c.as_str() == "+" || c.as_str() == "=" =>
                    {
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

/// Blit a rendered page into the framebuffer, centered on a neutral
/// background, then overlay search hits. Pixels are packed little-endian as
/// `B | (G << 8) | (R << 16)` per `softbuffer`'s contract (alpha is ignored
/// on the common platforms).
fn blit(
    page: &RenderedPage,
    buffer: &mut [u32],
    win_w: usize,
    win_h: usize,
    ctm: &Matrix,
    hits: &[Quad],
    hit_index: usize,
) {
    const BG: u32 = 0x2b2b2b; // dark gray backdrop
    buffer.fill(BG);

    let (pw, ph) = (page.width, page.height);
    if pw == 0 || ph == 0 || buffer.len() < win_w * win_h {
        return;
    }
    let off_x = ((win_w.saturating_sub(pw)) / 2) as isize;
    let off_y = ((win_h.saturating_sub(ph)) / 2) as isize;

    let px = &page.rgba;
    let (br, bg, bb) = ((BG & 0xff), ((BG >> 8) & 0xff), ((BG >> 16) & 0xff));
    for row in 0..ph {
        let dy = row as isize + off_y;
        if dy < 0 || dy as usize >= win_h {
            continue;
        }
        for col in 0..pw {
            let dx = col as isize + off_x;
            if dx < 0 || dx as usize >= win_w {
                continue;
            }
            let i = (row * pw + col) * 4;
            let r = px[i] as u32;
            let g = px[i + 1] as u32;
            let b = px[i + 2] as u32;
            let a = px[i + 3] as u32;
            // Alpha compositing over the backdrop for partial transparency.
            let r = (r * a + br * (255 - a)) / 255;
            let g = (g * a + bg * (255 - a)) / 255;
            let b = (b * a + bb * (255 - a)) / 255;
            let dst = dy as usize * win_w + dx as usize;
            buffer[dst] = b | (g << 8) | (r << 16);
        }
    }

    // Search-hit highlights: transform each PDF-space quad into pixel space
    // via the render matrix, then fill the bounding box with translucent
    // yellow. The "current" hit is drawn more opaque.
    for (idx, quad) in hits.iter().enumerate() {
        let ul = quad.ul.transform(ctm);
        let ur = quad.ur.transform(ctm);
        let ll = quad.ll.transform(ctm);
        let lr = quad.lr.transform(ctm);
        let min_x = ul.x.min(ur.x).min(ll.x).min(lr.x);
        let max_x = ul.x.max(ur.x).max(ll.x).max(lr.x);
        let min_y = ul.y.min(ur.y).min(ll.y).min(lr.y);
        let max_y = ul.y.max(ur.y).max(ll.y).max(lr.y);
        let (x0, x1) = (
            (min_x + off_x as f32).floor() as isize,
            (max_x + off_x as f32).ceil() as isize,
        );
        let (y0, y1) = (
            (min_y + off_y as f32).floor() as isize,
            (max_y + off_y as f32).ceil() as isize,
        );
        let alpha = if idx == hit_index { 150u32 } else { 90u32 };
        for yy in y0.max(0)..y1.min(win_h as isize) {
            for xx in x0.max(0)..x1.min(win_w as isize) {
                let dst = yy as usize * win_w + xx as usize;
                let cur = buffer[dst];
                let cr = cur & 0xff;
                let cg = (cur >> 8) & 0xff;
                let cb = (cur >> 16) & 0xff;
                // Blend yellow (255,255,0) over current pixel.
                let r = (255 * alpha + cr * (255 - alpha)) / 255;
                let g = (255 * alpha + cg * (255 - alpha)) / 255;
                let b = (cb * (255 - alpha)) / 255;
                buffer[dst] = b | (g << 8) | (r << 16);
            }
        }
    }
}
