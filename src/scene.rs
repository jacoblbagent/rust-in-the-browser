//! A lit, depth-tested 3D surface generated from a parametric curve and drawn
//! with WebGL2. No engine, no third-party crate: the geometry, the frames, the
//! shading and the draw call are all in this file.

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlProgram, WebGlBuffer, WebGlVertexArrayObject};

use crate::gl::{self, *};

const VERT: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_norm;

uniform float u_yaw;
uniform float u_pitch;
uniform float u_dist;
uniform float u_focal;
uniform float u_aspect;

out vec3 v_pos;
out vec3 v_norm;

mat3 rot_y(float a) {
    float c = cos(a), s = sin(a);
    return mat3(c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c);
}
mat3 rot_x(float a) {
    float c = cos(a), s = sin(a);
    return mat3(1.0, 0.0, 0.0, 0.0, c, s, 0.0, -s, c);
}

void main() {
    mat3 m = rot_y(u_yaw) * rot_x(u_pitch);
    vec3 p = m * a_pos;
    vec3 n = m * a_norm;

    float d = u_dist - p.z;
    gl_Position = vec4(p.x * u_focal / u_aspect, p.y * u_focal, -p.z, d);

    v_pos = p;
    v_norm = n;
}
"#;

const FRAG: &str = r#"#version 300 es
precision highp float;

in vec3 v_pos;
in vec3 v_norm;
out vec4 outColor;

const vec3 BASE = vec3(0.72, 0.26, 0.11);
const vec3 KEY  = vec3(1.00, 0.86, 0.68);
const vec3 FILL = vec3(0.35, 0.40, 0.52);
const vec3 RIM  = vec3(1.00, 0.58, 0.30);

void main() {
    vec3 n = normalize(v_norm);
    vec3 view_dir = normalize(vec3(0.0, 0.0, 1.0));

    vec3 key_l = normalize(vec3(-0.55, 0.75, 0.70));
    vec3 fill_l = normalize(vec3(0.80, -0.25, 0.45));
    vec3 rim_l = normalize(vec3(0.30, -0.55, -0.85));

    float key = max(dot(n, key_l), 0.0);
    float fill = max(dot(n, fill_l), 0.0) * 0.45;
    float rim = pow(1.0 - max(dot(n, view_dir), 0.0), 2.6);

    vec3 col = BASE * (0.10 + 0.95 * key) + BASE * fill * FILL + RIM * rim * 0.55;

    // Blinn-Phong highlight so the surface reads as a solid, not a flat fill.
    vec3 h = normalize(key_l + view_dir);
    float spec = pow(max(dot(n, h), 0.0), 46.0);
    col += KEY * spec * 0.7;

    // Slight fresnel lift keeps the silhouette from going to black.
    col += BASE * 0.12 * pow(1.0 - max(dot(n, view_dir), 0.0), 4.0);

    outColor = vec4(col, 1.0);
}
"#;

#[wasm_bindgen]
pub struct Knot {
    gl: Gl,
    canvas: HtmlCanvasElement,
    prog: WebGlProgram,
    vao: WebGlVertexArrayObject,
    _vbo: WebGlBuffer,
    _ibo: WebGlBuffer,
    indices: i32,
    verts: i32,
    yaw: f32,
    pitch: f32,
    auto: bool,
}

impl Knot {
    /// Build the tube surface: a ring of `sides` vertices swept along the curve,
    /// carried by a parallel-transport frame so the tube never pinches.
    fn tube(p: f32, q: f32, segs: usize, sides: usize, radius: f32) -> (Vec<f32>, Vec<u16>) {
        let curve = |t: f32| -> [f32; 3] {
            let (sp, cp) = (p * t).sin_cos();
            let (sq, cq) = (q * t).sin_cos();
            let r = 2.0 + cq;
            [r * cp / 3.0, r * sp / 3.0, sq / 3.0]
        };

        let mut verts: Vec<f32> = Vec::with_capacity(segs * sides * 6);
        let nrm = |v: &[f32; 3]| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            [v[0] / l, v[1] / l, v[2] / l]
        };

        // Seed the frame off the first tangent, then carry it along.
        let t0 = std::f32::consts::TAU;
        let a = curve(0.0);
        let b = curve(0.01);
        let mut tangent = nrm(&[b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
        let mut normal = {
            let up = [0.0f32, 0.0, 1.0];
            let d = tangent[0] * up[0] + tangent[1] * up[1] + tangent[2] * up[2];
            nrm(&[
                up[0] - d * tangent[0],
                up[1] - d * tangent[1],
                up[2] - d * tangent[2],
            ])
        };
        let _ = t0;

        for s in 0..segs {
            let t = (s as f32 / segs as f32) * std::f32::consts::TAU;
            let pt = curve(t);

            // Parallel transport: project the previous normal onto the new
            // plane perpendicular to the tangent, then re-orthonormalise.
            let nxt = curve(t + 0.004);
            let new_t = nrm(&[nxt[0] - pt[0], nxt[1] - pt[1], nxt[2] - pt[2]]);
            let d = normal[0] * new_t[0] + normal[1] * new_t[1] + normal[2] * new_t[2];
            let mut n = nrm(&[
                normal[0] - d * new_t[0],
                normal[1] - d * new_t[1],
                normal[2] - d * new_t[2],
            ]);
            let binormal = [
                new_t[1] * n[2] - new_t[2] * n[1],
                new_t[2] * n[0] - new_t[0] * n[2],
                new_t[0] * n[1] - new_t[1] * n[0],
            ];
            tangent = new_t;
            normal = n;
            let _ = &mut n;

            for k in 0..sides {
                let a = (k as f32 / sides as f32) * std::f32::consts::TAU;
                let (sa, ca) = a.sin_cos();
                let dir = [
                    n[0] * ca + binormal[0] * sa,
                    n[1] * ca + binormal[1] * sa,
                    n[2] * ca + binormal[2] * sa,
                ];
                verts.push(pt[0] + dir[0] * radius);
                verts.push(pt[1] + dir[1] * radius);
                verts.push(pt[2] + dir[2] * radius);
                // Outward radial direction is the surface normal.
                verts.push(dir[0]);
                verts.push(dir[1]);
                verts.push(dir[2]);
            }
        }

        let mut idx: Vec<u16> = Vec::with_capacity(segs * sides * 6);
        for s in 0..segs {
            let s1 = (s + 1) % segs;
            for k in 0..sides {
                let k1 = (k + 1) % sides;
                let a = (s * sides + k) as u16;
                let b = (s * sides + k1) as u16;
                let c = (s1 * sides + k1) as u16;
                let d = (s1 * sides + k) as u16;
                idx.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }
        let _ = tangent;
        (verts, idx)
    }
}

#[wasm_bindgen]
impl Knot {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<Knot, JsValue> {
        let (canvas, gl) = gl::context(canvas_id)?;
        let prog = gl::program(&gl, VERT, FRAG, &["a_pos", "a_norm"])?;

        gl.enable(DEPTH_TEST);
        gl.enable(CULL_FACE);
        gl.cull_face(BACK);

        let (verts, idx) = Self::tube(2.0, 3.0, 360, 20, 0.30);

        let vao = gl.create_vertex_array().ok_or("create_vertex_array failed")?;
        let vbo = gl.create_buffer().ok_or("create_buffer failed")?;
        let ibo = gl.create_buffer().ok_or("create_buffer failed")?;

        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(ARRAY_BUFFER, Some(&vbo));
        gl::upload_f32(&gl, ARRAY_BUFFER, &verts, STATIC_DRAW);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, FLOAT, false, 24, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 3, FLOAT, false, 24, 12);

        gl.bind_buffer(ELEMENT_ARRAY_BUFFER, Some(&ibo));
        gl::upload_u16(&gl, ELEMENT_ARRAY_BUFFER, &idx, STATIC_DRAW);

        Ok(Knot {
            gl,
            canvas,
            prog,
            vao,
            _vbo: vbo,
            _ibo: ibo,
            indices: idx.len() as i32,
            verts: (verts.len() / 6) as i32,
            yaw: 0.6,
            pitch: 0.35,
            auto: true,
        })
    }

    pub fn vertices(&self) -> i32 {
        self.verts
    }

    pub fn triangles(&self) -> i32 {
        self.indices / 3
    }

    /// Stop the auto-spin once the visitor drags the model themselves.
    pub fn set_auto(&mut self, on: bool) {
        self.auto = on;
    }

    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.3, 1.3);
    }

    pub fn frame(&mut self, dt: f32) {
        if self.auto {
            self.yaw += dt * 0.32;
            self.pitch = 0.35 + (self.yaw * 0.31).sin() * 0.28;
        }

        let (w, h) = gl::fit(&self.canvas, &self.gl, 2.0);
        self.gl.clear_color(0.043, 0.039, 0.041, 1.0);
        self.gl.clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT);
        self.gl.use_program(Some(&self.prog));
        self.gl.bind_vertex_array(Some(&self.vao));

        let set = |name: &str, v: f32| {
            if let Some(l) = self.gl.get_uniform_location(&self.prog, name) {
                self.gl.uniform1f(Some(&l), v);
            }
        };
        set("u_yaw", self.yaw);
        set("u_pitch", self.pitch);
        set("u_dist", 3.0);
        set("u_focal", 1.62);
        set("u_aspect", w as f32 / h.max(1) as f32);

        self.gl
            .draw_elements_with_i32(TRIANGLES, self.indices, UNSIGNED_SHORT, 0);
    }
}
