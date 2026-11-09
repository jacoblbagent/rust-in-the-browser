//! Conway's Game of Life on a grid far larger than anything a per-cell
//! JavaScript loop could step at interactive rates.
//!
//! Cells carry heat rather than a boolean, so the render shows where life has
//! just been. That costs one extra f32 per cell and makes the state legible.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Life {
    w: i32,
    h: i32,
    cur: Vec<u8>,
    next: Vec<u8>,
    heat: Vec<f32>,
    generation: u64,
    population: u32,
    seed: u64,
    buf: Vec<u8>,
}

fn rng(state: &mut u64) -> u64 {
    // xorshift64*, so the soup looks the same on every machine for a given seed.
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

impl Life {
    fn seed_soup(&mut self, density: f64) {
        for c in self.cur.iter_mut() {
            *c = ((rng(&mut self.seed) >> 11) as f64 / ((1u64 << 53) as f64) < density) as u8;
        }
        self.heat.iter_mut().for_each(|h| *h = 0.0);
        self.generation = 0;
        self.population = self.cur.iter().filter(|&&c| c == 1).count() as u32;
    }
}

#[wasm_bindgen]
impl Life {
    #[wasm_bindgen(constructor)]
    pub fn new(w: i32, h: i32, seed: u32) -> Life {
        let n = (w as usize) * (h as usize);
        let mut l = Life {
            w,
            h,
            cur: vec![0; n],
            next: vec![0; n],
            heat: vec![0.0; n],
            generation: 0,
            population: 0,
            seed: (seed as u64) | 1,
            buf: vec![0; n * 4],
        };
        l.seed_soup(0.14);
        l
    }

    pub fn pixels(&self) -> js_sys::Uint8ClampedArray {
        unsafe { js_sys::Uint8ClampedArray::view(&self.buf) }
    }

    pub fn width(&self) -> i32 {
        self.w
    }

    pub fn height(&self) -> i32 {
        self.h
    }

    pub fn cells(&self) -> u32 {
        (self.w * self.h) as u32
    }

    pub fn generation(&self) -> f64 {
        self.generation as f64
    }

    pub fn population(&self) -> u32 {
        self.population
    }

    pub fn reseed(&mut self) {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1) | 1;
        self.seed_soup(0.14);
    }

    /// One generation. Eight neighbours per cell, wrapped edges.
    pub fn step(&mut self) {
        let w = self.w;
        let h = self.h;
        let mut alive = 0u32;

        for y in 0..h {
            let row = (y as usize) * (w as usize);
            for x in 0..w {
                let up = ((y - 1).rem_euclid(h) as usize) * (w as usize);
                let mid = row;
                let dn = ((y + 1).rem_euclid(h) as usize) * (w as usize);
                let lx = ((x - 1).rem_euclid(w)) as usize;
                let rx = ((x + 1).rem_euclid(w)) as usize;
                let cx = x as usize;

                let n = self.cur[up + lx]
                    + self.cur[up + cx]
                    + self.cur[up + rx]
                    + self.cur[mid + lx]
                    + self.cur[mid + rx]
                    + self.cur[dn + lx]
                    + self.cur[dn + cx]
                    + self.cur[dn + rx];
                let here = self.cur[mid + cx];
                let live = if here == 1 { n == 2 || n == 3 } else { n == 3 };
                let v = live as u8;
                self.next[mid + cx] = v;
                alive += v as u32;
            }
        }

        std::mem::swap(&mut self.cur, &mut self.next);
        self.population = alive;
        self.generation += 1;

        // Heat decays slowly; a cell that just died reads as an ember.
        for i in 0..self.heat.len() {
            let h = self.heat[i] * 0.90;
            self.heat[i] = if self.cur[i] == 1 { 1.0 } else { h };
        }
    }

    /// Paint heat into the RGBA buffer.
    pub fn render(&mut self) {
        for i in 0..self.heat.len() {
            let t = self.heat[i];
            let o = i * 4;
            if t <= 0.02 {
                self.buf[o] = 12;
                self.buf[o + 1] = 11;
                self.buf[o + 2] = 12;
            } else {
                // Ember ramp, older cells cooling toward the surface colour.
                let t = t.clamp(0.0, 1.0);
                self.buf[o] = (18.0 + 237.0 * t * t) as u8;
                self.buf[o + 1] = (16.0 + 150.0 * t.powi(3)) as u8;
                self.buf[o + 2] = (16.0 + 62.0 * t.powi(4)) as u8;
            }
            self.buf[o + 3] = 255;
        }
    }
}
