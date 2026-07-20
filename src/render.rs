/// A rendered page: RGBA8 pixels, row-major.
///
/// `rgba` has length `width * height * 4`. Each pixel is
/// `(r, g, b, a)` in that byte order.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl RenderedPage {
    /// Length of the pixel buffer in `u32`s (one per pixel).
    #[allow(dead_code)]
    pub fn pixel_count(&self) -> usize {
        self.width * self.height
    }

    /// Convert the RGBA8 buffer into a `u32` framebuffer suitable for
    /// `softbuffer` on little-endian machines (memory order R,G,B,A).
    ///
    /// `softbuffer`'s `&[u32]` is interpreted as native-endian, where the
    /// low byte is red. Packing `a<<24 | b<<16 | g<<8 | r` yields R,G,B,A
    /// in memory on x86/ARM (little-endian), which is what the compositor
    /// expects.
    #[allow(dead_code)]
    pub fn to_argb_u32(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.pixel_count());
        let px = &self.rgba;
        for chunk in px.chunks_exact(4) {
            let r = chunk[0] as u32;
            let g = chunk[1] as u32;
            let b = chunk[2] as u32;
            let a = chunk[3] as u32;
            out.push((a << 24) | (b << 16) | (g << 8) | r);
        }
        out
    }
}

use mupdf::{Matrix, Quad};

/// Build the render/transform matrix matching `crate::document::ViewerDocument::render`.
pub fn ctm_for(scale: f32, rotation: u8) -> Matrix {
    let mut ctm = Matrix::new_scale(scale, scale);
    if rotation != 0 {
        ctm.concat(Matrix::new_rotate(rotation as f32));
    }
    ctm
}

/// Blend translucent yellow over the RGBA page buffer at the given hits.
///
/// Each hit `Quad` is in PDF coordinates; `ctm` maps it into pixel space.
/// The "current" hit (`hit_index`) is drawn more opaque than the rest.
/// This mutates the page's RGBA buffer in place and is used both by the GUI
/// viewer and by the headless CLI renderer.
pub fn apply_highlights(
    rgba: &mut [u8],
    w: usize,
    h: usize,
    ctm: &Matrix,
    hits: &[Quad],
    hit_index: usize,
) {
    for (idx, quad) in hits.iter().enumerate() {
        let ul = quad.ul.transform(ctm);
        let ur = quad.ur.transform(ctm);
        let ll = quad.ll.transform(ctm);
        let lr = quad.lr.transform(ctm);
        let min_x = ul.x.min(ur.x).min(ll.x).min(lr.x);
        let max_x = ul.x.max(ur.x).max(ll.x).max(lr.x);
        let min_y = ul.y.min(ur.y).min(ll.y).min(lr.y);
        let max_y = ul.y.max(ur.y).max(ll.y).max(lr.y);
        let (x0, x1) = (min_x.floor() as isize, max_x.ceil() as isize);
        let (y0, y1) = (min_y.floor() as isize, max_y.ceil() as isize);
        let alpha = if idx == hit_index { 150u32 } else { 90u32 };
        for yy in y0.max(0)..y1.min(h as isize) {
            for xx in x0.max(0)..x1.min(w as isize) {
                let i = ((yy as usize) * w + (xx as usize)) * 4;
                let cr = rgba[i] as u32;
                let cg = rgba[i + 1] as u32;
                let cb = rgba[i + 2] as u32;
                // Blend yellow (255,255,0) over the existing pixel.
                rgba[i] = ((255 * alpha + cr * (255 - alpha)) / 255) as u8;
                rgba[i + 1] = ((255 * alpha + cg * (255 - alpha)) / 255) as u8;
                rgba[i + 2] = ((cb * (255 - alpha)) / 255) as u8;
            }
        }
    }
}

/// Composite a rendered page (plus optional search highlights) onto a
/// `softbuffer` `u32` framebuffer, centered on a neutral gray background.
pub fn blit_to_buffer(
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

    // Apply highlights onto a copy of the page's own RGBA (keeps the
    // original buffer pristine for hit cycling without re-rendering).
    let mut rgba = page.rgba.clone();
    apply_highlights(&mut rgba, pw, ph, ctm, hits, hit_index);

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
            let r = rgba[i] as u32;
            let g = rgba[i + 1] as u32;
            let b = rgba[i + 2] as u32;
            let a = rgba[i + 3] as u32;
            // Alpha compositing over the backdrop for partial transparency.
            let r = (r * a + br * (255 - a)) / 255;
            let g = (g * a + bg * (255 - a)) / 255;
            let b = (b * a + bb * (255 - a)) / 255;
            let dst = dy as usize * win_w + dx as usize;
            buffer[dst] = b | (g << 8) | (r << 16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mupdf::Point;

    #[test]
    fn ctm_scale_only_is_axis_aligned() {
        let m = ctm_for(2.0, 0);
        // No rotation: a unit point maps to (2,2) with no shear.
        let p = Point::new(1.0, 1.0).transform(&m);
        assert!((p.x - 2.0).abs() < 1e-3 && (p.y - 2.0).abs() < 1e-3);
    }

    #[test]
    fn ctm_rotation_90_maps_axes() {
        let m = ctm_for(1.0, 90);
        let p = Point::new(1.0, 0.0).transform(&m);
        // MuPDF's rotate(90) maps (1,0) -> (0,1); assert it swaps axes and
        // stays unit length (orthogonal, non-shearing rotation).
        assert!(
            (p.x).abs() < 1e-3 && (p.y - 1.0).abs() < 1e-3,
            "got {:?}",
            p
        );
        let len = (p.x * p.x + p.y * p.y).sqrt();
        assert!((len - 1.0).abs() < 1e-3);
    }

    #[test]
    fn apply_highlights_paints_yellow_and_leaves_rest() {
        // 4x4 image, all transparent black.
        let mut rgba = vec![0u8; 4 * 4 * 4];
        // A hit quad covering pixel (1,1) at 1x scale (no rotation).
        let q = Quad {
            ul: Point::new(1.0, 1.0),
            ur: Point::new(2.0, 1.0),
            ll: Point::new(1.0, 2.0),
            lr: Point::new(2.0, 2.0),
        };
        let m = ctm_for(1.0, 0);
        apply_highlights(&mut rgba, 4, 4, &m, &[q], 0);
        // The single covered pixel must be yellow-ish: r=g high, b=0.
        let i = 20;
        assert!(
            rgba[i] > 100 && rgba[i + 1] > 100 && rgba[i + 2] == 0,
            "hit px = {:?}",
            &rgba[i..i + 3]
        );
        // A far pixel must be untouched.
        let j = (3 * 4 + 3) * 4;
        assert_eq!(&rgba[j..j + 3], &[0, 0, 0]);
    }

    #[test]
    fn apply_highlights_current_is_more_opaque() {
        let mut a = vec![0u8; 4 * 2 * 2];
        let mut b = vec![0u8; 4 * 2 * 2];
        let q = Quad {
            ul: Point::new(0.0, 0.0),
            ur: Point::new(2.0, 0.0),
            ll: Point::new(0.0, 2.0),
            lr: Point::new(2.0, 2.0),
        };
        let m = ctm_for(1.0, 0);
        let hits = [q];
        apply_highlights(&mut a, 2, 2, &m, &hits, 0); // current
        apply_highlights(&mut b, 2, 2, &m, &hits, 99); // not current
                                                       // Current hit blends at alpha 150 vs 90 -> higher red channel.
        assert!(a[0] > b[0]);
    }

    #[test]
    fn blit_centers_page_and_skips_oob() {
        let page = RenderedPage {
            width: 2,
            height: 2,
            rgba: vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let mut buf = vec![0u32; 4 * 4]; // 4x4 window, page is 2x2 -> centered
        blit_to_buffer(&page, &mut buf, 4, 4, &ctm_for(1.0, 0), &[], 0);
        // Backdrop is 0x2b2b2b; an opaque red pixel must show up somewhere.
        // Packing is b|(g<<8)|(r<<16), so red 255 sits in the high byte.
        let reds: usize = buf
            .iter()
            .filter(|&&p| ((p >> 16) & 0xff) == 255 && ((p >> 8) & 0xff) == 0)
            .count();
        assert_eq!(reds, 1);
        // The rest are the gray backdrop.
        let grays: usize = buf.iter().filter(|&&p| p == 0x2b2b2b).count();
        assert_eq!(grays, 15);
    }

    #[test]
    fn blit_guards_against_tiny_buffer() {
        let page = RenderedPage {
            width: 10,
            height: 10,
            rgba: vec![0; 10 * 10 * 4],
        };
        let mut buf = vec![0x2b2b2bu32; 4]; // too small for a 10x10 page
        blit_to_buffer(&page, &mut buf, 2, 2, &ctm_for(1.0, 0), &[], 0);
        // Must not panic and must leave the buffer as the filled backdrop.
        assert!(buf.iter().all(|&p| p == 0x2b2b2b));
    }
}
