//! Rust in the browser: four compute-bound demos in WebAssembly.
//!
//! Every module below owns its own canvas work. Nothing here calls back into
//! JavaScript on a hot path.

mod galaxy;
mod gl;
mod life;
mod mandel;
mod scene;

use wasm_bindgen::prelude::*;

pub use galaxy::Galaxy;
pub use life::Life;
pub use mandel::Mandelbrot;
pub use scene::Knot;

/// Trap panics into the console instead of a bare "unreachable" error.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Build facts, captured at compile time by `build.rs`.
#[wasm_bindgen]
pub fn toolchain() -> String {
    env!("RUSTC_VERSION").to_string()
}

#[wasm_bindgen]
pub fn build_epoch() -> f64 {
    env!("BUILD_EPOCH").parse::<f64>().unwrap_or(0.0)
}

/// Target feature detection, reported on the page rather than assumed.
#[wasm_bindgen]
pub fn has_simd() -> bool {
    cfg!(target_feature = "simd128")
}

/// A hand-rolled timing loop, exported so the page can ask the module to run
/// the fractal benchmark on the Rust side of the wall. `wide` selects the
/// two-lane kernel.
#[wasm_bindgen]
pub fn bench_mandelbrot(iters: u32, max_iter: u32, w: u32, h: u32, wide: bool) -> f64 {
    let mut m = Mandelbrot::new(w, h);
    m.set_max_iter(max_iter);
    m.set_phase(0.0);
    // Warm up so the first-run allocation cost is not in the reported figure.
    m.render();
    m.render_wide();
    let t0 = now();
    for _ in 0..iters {
        if wide {
            m.render_wide();
        } else {
            m.render();
        }
    }
    (now() - t0) / iters as f64
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}
