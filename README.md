# Rust in the browser

A single-page site with four compute-heavy systems written in Rust, compiled to
WebAssembly, and run live in the page. There is no engine underneath any of
them: the geometry, the frames, the shading and the draw calls are all in the
crate.

    www/          the page, its stylesheet and its script
    src/          the Rust crate
    build.sh      builds the wasm package into www/pkg

## What runs

| Section | What Rust does |
| --- | --- |
| Fractal | Escape-time loop per pixel, a scalar kernel and a two-lane `f64x2` kernel, plus a JavaScript baseline written to match the scalar kernel statement for statement. The page compares the three buffers byte for byte and times them against each other. |
| Galaxy | 200,000 point masses on circular orbits under a softened central mass, integrated with a symplectic Euler step. Rust owns the WebGL2 context, the vertex buffer and the draw call. |
| Life | Conway's Game of Life on a 1,200 by 900 grid with wrapping edges. Rust steps the grid and paints the RGBA buffer, leaving a cooling ember where a cell has just died. |
| Surface | A (2,3) torus knot. Rust sweeps a tube along the curve with a parallel-transport frame, builds the index buffer, and shades it with a key light, a fill and a rim. |

## Building

Everything runs from a user-local toolchain. No root, nothing installed
system-wide.

    rustup target add wasm32-unknown-unknown
    cargo install wasm-pack
    ./build.sh

`build.sh` puts `+simd128` in `RUSTFLAGS`, which is what lets LLVM emit the
four-wide wasm vector instructions. `build.rs` captures the compiler version
and the build time at compile time, so the figures the page prints are read
back from the module rather than written into the markup.

## Running

    python3 -m http.server 5185 --bind 127.0.0.1 --directory www

Then open `http://127.0.0.1:5185/`. The module has to be served over HTTP; it
will not load from a `file://` URL.

## Notes on the code

- GL enum values are written as literals rather than taken from `web-sys`.
  Those constants move between releases, and keeping them out keeps the
  `web-sys` feature surface small, which keeps the module small.
- Pixels are shared with JavaScript without a copy: Rust hands back a
  `Uint8ClampedArray` view over its own buffer and the page wraps it in an
  `ImageData`.
- The fractal palette avoids `ln` and `log2`. Those pull a software libm
  implementation into the module, which costs more than the fractal itself, so
  the colour curve is built from multiplies and `sqrt`.
- The galaxy's seeded orbital speed uses the softened potential
  (`v = r / (r^2 + a^2)^0.75`), not the point-mass one. With the point-mass
  formula the innermost bodies get far more speed than a closed orbit needs and
  the core empties itself within a few seconds.
- `window.__rust` exposes the module and the live instances. The browser test
  suite uses it to read state directly instead of scraping the DOM.

## Verification

The suite is Playwright driving headless Chromium with SwiftShader, and covers
the module load, every control, the three-way benchmark and its byte
comparison, the nav, and the absence of page, console and request errors. It
also reads the canvases back to confirm each one renders real content rather
than a flat fill.
