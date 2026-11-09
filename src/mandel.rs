//! Escape-time fractal, evaluated on the CPU for every pixel.
//!
//! Two kernels live here: a scalar one that matches the JavaScript baseline in
//! `www/app.js` statement for statement, and a two-lane one built on the wasm
//! `simd128` vector instructions. The scalar kernel is the honest control for
//! the head-to-head benchmark; the vector kernel is what the live view uses.
//!
//! The colouring deliberately avoids `ln`/`log2`: those pull a software libm
//! implementation into the module, which costs more than the fractal itself.
//! Everything here is multiply, add and `sqrt`, all of which are hardware
//! instructions on both sides.

use wasm_bindgen::prelude::*;

/// Colour ramp, warm charcoal through rust to hot cream.
const RAMP: [(f32, f32, f32); 6] = [
    (26.0, 17.0, 18.0),
    (62.0, 28.0, 28.0),
    (128.0, 49.0, 28.0),
    (214.0, 104.0, 48.0),
    (246.0, 178.0, 110.0),
    (255.0, 236.0, 214.0),
];

/// Points inside the set. Sits just off the dark end of the ramp, so the body
/// of the set reads as a shape against the bright band that hugs the boundary
/// rather than merging with the far exterior.
const INTERIOR: [u8; 4] = [44, 24, 22, 255];

#[inline]
fn ramp(t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32;
    let i = (t as usize).min(RAMP.len() - 2);
    let f = t - i as f32;
    let (r0, g0, b0) = RAMP[i];
    let (r1, g1, b1) = RAMP[i + 1];
    (
        (r0 + (r1 - r0) * f) as u8,
        (g0 + (g1 - g0) * f) as u8,
        (b0 + (b1 - b0) * f) as u8,
    )
}

/// Iteration count to a colour. Kept in one place so the scalar and vector
/// kernels cannot drift apart.
///
/// The count is normalised, then bent by `sqrt(sqrt(u))` (that is, `u^0.25`).
/// Most of the frame escapes in only a handful of iterations, so without a
/// bend like this the whole exterior collapses onto the darkest ramp stop;
/// this spreads the low counts across the ramp and leaves the far field dark
/// while the boundary band runs up to cream. `sqrt` is a hardware instruction
/// on both sides, so no libm code ends up in the module.
#[inline]
fn shade(i: u32, max_iter: u32, inv: f64, gain: f32) -> [u8; 4] {
    if i >= max_iter {
        return INTERIOR;
    }
    let u = ((i as f64) * inv) as f32;
    let (r, g, b) = ramp(u.sqrt().sqrt() * gain);
    [r, g, b, 255]
}

#[wasm_bindgen]
pub struct Mandelbrot {
    w: u32,
    h: u32,
    max_iter: u32,
    scale: f64,
    cx: f64,
    cy: f64,
    phase: f64,
    buf: Vec<u8>,
}

#[wasm_bindgen]
impl Mandelbrot {
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32) -> Mandelbrot {
        Mandelbrot {
            w,
            h,
            max_iter: 320,
            // Window width 2.9 centred at -0.72 puts the set's mass near the
            // middle of the frame instead of hard against the right edge.
            scale: 2.9 / w as f64,
            cx: -0.72,
            cy: 0.0,
            phase: 0.35,
            buf: vec![0; (w as usize) * (h as usize) * 4],
        }
    }

    /// Slow brightness drift, derived from the phase with a triangle wave so
    /// no transcendental ends up in the per-pixel path. Exported so the
    /// JavaScript baseline runs on the same exposure.
    pub fn gain(&self) -> f32 {
        let p = self.phase as f32;
        let tri = 1.0 - (2.0 * p - 1.0).abs();
        0.80 + 0.20 * tri
    }

    /// Zero-copy view of the RGBA buffer. wasm-bindgen hands this back as a
    /// plain byte array, so the page wraps it as a clamped array before it
    /// becomes an ImageData.
    pub fn pixels(&self) -> js_sys::Uint8ClampedArray {
        unsafe { js_sys::Uint8ClampedArray::view(&self.buf) }
    }

    pub fn byte_len(&self) -> usize {
        self.buf.len()
    }

    pub fn width(&self) -> u32 {
        self.w
    }

    pub fn height(&self) -> u32 {
        self.h
    }

    pub fn max_iter(&self) -> u32 {
        self.max_iter
    }

    pub fn set_max_iter(&mut self, n: u32) {
        self.max_iter = n.clamp(16, 4096);
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.scale = 2.9 / (self.w as f64 * zoom.max(1.0));
    }

    /// Read back the live window, so the JavaScript baseline always runs on the
    /// same rectangle the Rust kernel is using.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn center_x(&self) -> f64 {
        self.cx
    }

    pub fn center_y(&self) -> f64 {
        self.cy
    }

    pub fn set_center(&mut self, cx: f64, cy: f64) {
        self.cx = cx;
        self.cy = cy;
    }

    /// Drift the palette so the still image keeps moving.
    pub fn advance(&mut self, dt: f64) {
        self.phase = (self.phase + dt * 0.06) % 1.0;
    }

    /// Pin the palette, so the benchmark compares a fixed image.
    pub fn set_phase(&mut self, p: f64) {
        self.phase = p;
    }

    /// One pixel at a time. This is the function the benchmark times.
    pub fn render(&mut self) {
        let w = self.w as usize;
        let h = self.h as usize;
        let max_iter = self.max_iter;
        let scale = self.scale;
        let cx0 = self.cx;
        let cy0 = self.cy;
        let inv = 1.0 / max_iter as f64;
        let gain = self.gain();
        let half_w = w as f64 * 0.5;
        let half_h = h as f64 * 0.5;

        for y in 0..h {
            let ci = cy0 + (y as f64 - half_h) * scale;
            let row = y * w * 4;
            for x in 0..w {
                let cr = cx0 + (x as f64 - half_w) * scale;
                let (mut zr, mut zi) = (0.0f64, 0.0f64);
                let (mut zr2, mut zi2) = (0.0f64, 0.0f64);
                let mut i = 0u32;
                while i < max_iter && zr2 + zi2 <= 4.0 {
                    zi = 2.0 * zr * zi + ci;
                    zr = zr2 - zi2 + cr;
                    zr2 = zr * zr;
                    zi2 = zi * zi;
                    i += 1;
                }
                let o = row + x * 4;
                self.buf[o..o + 4].copy_from_slice(&shade(i, max_iter, inv, gain));
            }
        }
    }

    /// The same image, two pixels per instruction. Falls back to the scalar
    /// kernel on a target without `simd128`.
    pub fn render_wide(&mut self) {
        #[cfg(target_feature = "simd128")]
        wide::render(self);

        #[cfg(not(target_feature = "simd128"))]
        self.render();
    }
}

#[cfg(target_feature = "simd128")]
mod wide {
    use super::{shade, Mandelbrot};
    use core::arch::wasm32::*;

    /// Two neighbouring pixels share one iteration loop. A lane that has
    /// already escaped stops counting but keeps being carried, so the pair
    /// costs whatever the slower pixel costs.
    pub fn render(m: &mut Mandelbrot) {
        let w = m.w as usize;
        let h = m.h as usize;
        let max_iter = m.max_iter;
        let scale = m.scale;
        let cx0 = m.cx;
        let cy0 = m.cy;
        let inv = 1.0 / max_iter as f64;
        let gain = m.gain();
        let half_w = w as f64 * 0.5;
        let half_h = h as f64 * 0.5;

        let two = f64x2_splat(2.0);
        let four = f64x2_splat(4.0);
        let zero = f64x2_splat(0.0);
        let one = i64x2_splat(1);

        for y in 0..h {
            let ci = cy0 + (y as f64 - half_h) * scale;
            let civ = f64x2_splat(ci);
            let row = y * w * 4;

            let mut x = 0usize;
            while x < w {
                let pair = x + 2 <= w;
                let cr0 = cx0 + (x as f64 - half_w) * scale;
                let cr1 = if pair {
                    cx0 + ((x + 1) as f64 - half_w) * scale
                } else {
                    cr0
                };
                let crv = f64x2(cr0, cr1);

                let mut zr = zero;
                let mut zi = zero;
                let mut zr2 = zero;
                let mut zi2 = zero;
                let mut counts = i64x2_splat(0);
                let mut done = i64x2_splat(0);

                let mut k = 0u32;
                while k < max_iter {
                    zi = f64x2_add(f64x2_mul(f64x2_mul(two, zr), zi), civ);
                    zr = f64x2_add(f64x2_sub(zr2, zi2), crv);
                    zr2 = f64x2_mul(zr, zr);
                    zi2 = f64x2_mul(zi, zi);
                    let mag = f64x2_add(zr2, zi2);

                    // Count before latching the escape, so a lane's total
                    // matches the scalar kernel's loop counter exactly.
                    counts = i64x2_add(counts, v128_andnot(one, done));
                    done = v128_or(done, f64x2_gt(mag, four));
                    if i64x2_all_true(done) {
                        break;
                    }
                    k += 1;
                }

                let n0 = i64x2_extract_lane::<0>(counts).max(0) as u32;
                let n1 = i64x2_extract_lane::<1>(counts).max(0) as u32;

                let o = row + x * 4;
                m.buf[o..o + 4].copy_from_slice(&shade(n0, max_iter, inv, gain));
                if pair {
                    m.buf[o + 4..o + 8].copy_from_slice(&shade(n1, max_iter, inv, gain));
                }
                x += if pair { 2 } else { 1 };
            }
        }
    }
}
