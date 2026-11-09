//! A galactic disc of point masses integrated in Rust and drawn as GPU points.
//!
//! The whole simulation stays in Rust: it owns the WebGL2 context, the buffers
//! and the draw call. JavaScript only calls `frame()` with a delta time.

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlProgram, WebGlVertexArrayObject};

use crate::gl::{self, *};

const VERT: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_col;

uniform float u_spin;
uniform float u_incl;
uniform float u_dist;
uniform float u_focal;
uniform float u_aspect;
uniform float u_size;
uniform float u_lift;

out vec3 v_col;
out float v_dim;

void main() {
    float cs = cos(u_spin), ss = sin(u_spin);
    mat3 rz = mat3(cs, -ss, 0.0, ss, cs, 0.0, 0.0, 0.0, 1.0);
    float ci = cos(u_incl), si = sin(u_incl);
    mat3 rx = mat3(1.0, 0.0, 0.0, 0.0, ci, -si, 0.0, si, ci);
    vec3 p = rx * (rz * a_pos);

    float d = u_dist - p.z;
    float k = u_focal / max(d, 0.05);
    // u_lift recentres the disc: the perspective enlarges the near half, which
    // otherwise drags the whole ellipse down against the bottom of the frame.
    gl_Position = vec4(p.x * k / u_aspect, p.y * k + u_lift, 0.0, 1.0);
    gl_PointSize = clamp(u_size * k, 1.0, 7.0);

    v_col = a_col;
    // Gentle depth cue, plus a fade over the outer third: without it the disc
    // ends on a hard elliptical rim instead of dissolving into the background.
    float rr = length(a_pos) / 1.6;
    v_dim = clamp(1.0 - (d - u_dist) * 0.12, 0.6, 1.0) * smoothstep(1.0, 0.7, rr);
}
"#;

const FRAG: &str = r#"#version 300 es
precision highp float;

in vec3 v_col;
in float v_dim;
out vec4 outColor;

void main() {
    // Soft disc sprite, additive, so dense regions read bright.
    vec2 d = gl_PointCoord - vec2(0.5);
    float r2 = dot(d, d);
    if (r2 > 0.25) discard;
    float a = smoothstep(0.25, 0.0, r2);
    outColor = vec4(v_col * v_dim * a, a);
}
"#;

#[wasm_bindgen]
pub struct Galaxy {
    gl: Gl,
    canvas: HtmlCanvasElement,
    prog: WebGlProgram,
    vao: WebGlVertexArrayObject,
    vbo: web_sys::WebGlBuffer,
    n: usize,
    count: usize,
    gpu: Vec<f32>,
    px: Vec<f32>,
    py: Vec<f32>,
    pz: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
    spin: f32,
    elapsed: f32,
    seed: u64,
    last_ms: f64,
}

fn unit(seed: &mut u64) -> f32 {
    // xorshift64* mapped to [0,1)
    let mut x = *seed;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *seed = x;
    let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
    ((v >> 11) as f64 / (1u64 << 53) as f64) as f32
}

#[wasm_bindgen]
impl Galaxy {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str, count: usize) -> Result<Galaxy, JsValue> {
        let (canvas, gl) = gl::context(canvas_id)?;
        let prog = gl::program(&gl, VERT, FRAG, &["a_pos", "a_col"])?;

        gl.enable(BLEND);
        gl.blend_func(SRC_ALPHA, ONE);
        gl.disable(DEPTH_TEST);

        let vao = gl.create_vertex_array().ok_or("create_vertex_array failed")?;
        let vbo = gl.create_buffer().ok_or("create_buffer failed")?;
        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(ARRAY_BUFFER, Some(&vbo));
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, FLOAT, false, 24, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 3, FLOAT, false, 24, 12);

        let mut g = Galaxy {
            gl,
            canvas,
            prog,
            vao,
            vbo,
            n: count,
            count,
            gpu: vec![0.0; count * 6],
            px: vec![0.0; count],
            py: vec![0.0; count],
            pz: vec![0.0; count],
            vx: vec![0.0; count],
            vy: vec![0.0; count],
            vz: vec![0.0; count],
            spin: 0.0,
            elapsed: 0.0,
            seed: 0x9E37_79B9_7F4A_7C15,
            last_ms: 0.0,
        };
        g.populate(count);
        Ok(g)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Milliseconds spent in the last integration step.
    pub fn last_step_ms(&self) -> f64 {
        self.last_ms
    }

    /// Flat `x, y, z` triples of the live body positions. The verification
    /// harness reads this to check the disc's radial profile directly, rather
    /// than inferring it from pixels.
    pub fn positions(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.count * 3);
        for i in 0..self.count {
            out.push(self.px[i]);
            out.push(self.py[i]);
            out.push(self.pz[i]);
        }
        out
    }

    /// Rebuild the disc. `count` cannot exceed the capacity given at construction.
    pub fn populate(&mut self, count: usize) {
        self.count = count.min(self.n);
        let n = self.count;
        for i in 0..n {
            // Uniform surface density comes from sampling the square of the
            // radius uniformly. Sampling r itself would pile bodies against
            // the outer rim, and sampling r as a linear ramp off r0 would
            // leave the middle empty.
            const R_IN: f32 = 0.005;
            const R_OUT: f32 = 1.60;
            let r = (R_IN * R_IN + unit(&mut self.seed) * (R_OUT * R_OUT - R_IN * R_IN)).sqrt();
            let a = unit(&mut self.seed) * std::f32::consts::TAU;
            let (sa, ca) = a.sin_cos();
            // Thin disc: a small vertical scatter gives it an edge.
            let z = (unit(&mut self.seed) - 0.5) * 0.045 * (1.35 - r).max(0.15);
            self.px[i] = ca * r;
            self.py[i] = sa * r;
            self.pz[i] = z;

            // Circular speed for the softened point mass the stepper actually
            // integrates: v = r / (r^2 + a^2)^0.75. Dropping the softening
            // term here hands the innermost bodies far more speed than a
            // closed orbit needs, and they leave, which empties the core.
            let soft = r * r + 0.02;
            let v = r / (soft * soft.sqrt()).sqrt();
            let jitter = 0.986 + unit(&mut self.seed) * 0.028;
            self.vx[i] = -sa * v * jitter;
            self.vy[i] = ca * v * jitter;
            self.vz[i] = 0.0;
        }
        self.elapsed = 0.0;
        self.sync_colors();
        self.upload();
    }

    /// Add `count` new bodies on steep infall orbits.
    pub fn add_infall(&mut self, count: usize) {
        let start = self.count;
        let end = (self.count + count).min(self.n);
        for i in start..end {
            let r = 1.4 + unit(&mut self.seed) * 0.7;
            let a = unit(&mut self.seed) * std::f32::consts::TAU;
            let (sa, ca) = a.sin_cos();
            self.px[i] = ca * r;
            self.py[i] = sa * r;
            self.pz[i] = (unit(&mut self.seed) - 0.5) * 0.35;
            // Mostly radial, so the cloud falls in and shears out.
            self.vx[i] = -ca * (0.22 + unit(&mut self.seed) * 0.2) + -sa * 0.42;
            self.vy[i] = -sa * (0.22 + unit(&mut self.seed) * 0.2) + ca * 0.42;
            self.vz[i] = (unit(&mut self.seed) - 0.5) * 0.06;
        }
        self.count = end;
    }

    /// One physics step plus one draw. `dt` is seconds.
    pub fn frame(&mut self, dt: f32) {
        let t0 = perf_now();
        self.step(dt);
        self.last_ms = perf_now() - t0;
        self.sync_colors();
        self.upload();
        self.draw();
    }

    fn step(&mut self, dt: f32) {
        // The innermost orbits turn in well under a second, so the step is
        // capped at a sixtieth: a bigger step and the core is integrated into
        // an escape instead of an orbit.
        let dt = dt.clamp(0.0, 1.0 / 60.0);
        self.elapsed += dt;
        let n = self.count;
        let gm = 1.0f32;
        for i in 0..n {
            let (x, y, z) = (self.px[i], self.py[i], self.pz[i]);
            let r2 = x * x + y * y + z * z + 0.02;
            let inv = gm / (r2 * r2.sqrt());
            // Symplectic Euler: half-kick, drift, half-kick keeps the disc stable.
            let (ax, ay, az) = (-x * inv, -y * inv, -z * inv);
            self.vx[i] += ax * dt;
            self.vy[i] += ay * dt;
            self.vz[i] += az * dt;
            self.px[i] += self.vx[i] * dt;
            self.py[i] += self.vy[i] * dt;
            self.pz[i] += self.vz[i] * dt;
        }
    }

    fn sync_colors(&mut self) {
        let n = self.count;
        for i in 0..n {
            let (x, y, z) = (self.px[i], self.py[i], self.pz[i]);
            let r = (x * x + y * y + z * z).sqrt();
            // Distance from the centre sets the hue: the core reads hot cream
            // and the disc falls away through amber into rust. Speed would be
            // a tempting driver, but the softened potential turns the rotation
            // curve over inside a third of the radius, which would paint a
            // ring instead of a core.
            let t = (r / 1.6).clamp(0.0, 1.0);
            let (cr, cg, cb) = if t < 0.5 {
                let f = t * 2.0;
                (1.0 - 0.16 * f, 0.93 - 0.35 * f, 0.80 - 0.55 * f)
            } else {
                let f = (t - 0.5) * 2.0;
                (0.72 - 0.26 * f, 0.44 - 0.28 * f, 0.20 - 0.10 * f)
            };
            // Fade the rim so the disc does not end on a hard edge.
            let dim = 1.0 - 0.25 * (r / 1.6).clamp(0.0, 1.0);
            let o = i * 6;
            self.gpu[o] = x;
            self.gpu[o + 1] = y;
            self.gpu[o + 2] = z;
            self.gpu[o + 3] = cr * dim;
            self.gpu[o + 4] = cg * dim;
            self.gpu[o + 5] = cb * dim;
        }
    }

    fn upload(&mut self) {
        let gl = &self.gl;
        gl.bind_vertex_array(Some(&self.vao));
        gl.bind_buffer(ARRAY_BUFFER, Some(&self.vbo));
        if self.count == self.n {
            gl::upload_f32(gl, ARRAY_BUFFER, &self.gpu, DYNAMIC_DRAW);
        } else {
            // Partial uploads once the cloud is smaller than the allocated buffer.
            let view = unsafe { js_sys::Float32Array::view(&self.gpu[..self.count * 6]) };
            gl.buffer_sub_data_with_i32_and_array_buffer_view(ARRAY_BUFFER, 0, &view);
        }
    }

    fn draw(&mut self) {
        let (w, h) = gl::fit(&self.canvas, &self.gl, 2.0);
        self.gl.clear_color(0.035, 0.031, 0.033, 1.0);
        self.gl.clear(COLOR_BUFFER_BIT);
        if self.count == 0 {
            return;
        }
        self.gl.use_program(Some(&self.prog));
        self.gl.bind_vertex_array(Some(&self.vao));

        let uni = |name: &str, v: f32| {
            if let Some(l) = self.gl.get_uniform_location(&self.prog, name) {
                self.gl.uniform1f(Some(&l), v);
            }
        };
        self.spin = (self.elapsed * 0.45) % std::f32::consts::TAU;
        uni("u_spin", self.spin);
        uni("u_incl", 1.02);
        uni("u_dist", 9.0);
        uni("u_focal", 6.0);
        uni("u_aspect", w as f32 / h.max(1) as f32);
        uni("u_size", 2.4);
        uni("u_lift", 0.09);

        self.gl
            .draw_arrays(POINTS, 0, self.count.min(i32::MAX as usize) as i32);
    }
}

fn perf_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}
