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
