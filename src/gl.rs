//! WebGL2 plumbing shared by the two GPU demos.
//!
//! GL enum values are written as literals on purpose: the `web-sys` constants
//! have moved around between releases, and literals keep the `web-sys` feature
//! surface small (which is what keeps the .wasm small).

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlProgram, WebGlShader,
};

// --- GL enums -------------------------------------------------------------
pub const POINTS: u32 = 0x0000;
pub const TRIANGLES: u32 = 0x0004;
pub const DEPTH_TEST: u32 = 0x0B71;
pub const CULL_FACE: u32 = 0x0B44;
pub const BLEND: u32 = 0x0BE2;
pub const BACK: u32 = 0x0405;
pub const SRC_ALPHA: u32 = 0x0302;
pub const ONE: u32 = 1;
pub const COLOR_BUFFER_BIT: u32 = 0x4000;
pub const DEPTH_BUFFER_BIT: u32 = 0x0100;
pub const ARRAY_BUFFER: u32 = 0x8892;
pub const ELEMENT_ARRAY_BUFFER: u32 = 0x8893;
pub const STATIC_DRAW: u32 = 0x88E4;
pub const DYNAMIC_DRAW: u32 = 0x88E8;
pub const FLOAT: u32 = 0x1406;
pub const UNSIGNED_SHORT: u32 = 0x1403;
pub const VERTEX_SHADER: u32 = 0x8B31;
pub const FRAGMENT_SHADER: u32 = 0x8B30;
pub const COMPILE_STATUS: u32 = 0x8B81;
pub const LINK_STATUS: u32 = 0x8B82;

/// Grab a WebGL2 context off a canvas by element id.
pub fn context(canvas_id: &str) -> Result<(HtmlCanvasElement, Gl), JsValue> {
    let win = web_sys::window().ok_or("no window")?;
    let doc = win.document().ok_or("no document")?;
    let el = doc
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("missing canvas #{canvas_id}")))?;
    let canvas: HtmlCanvasElement = el.dyn_into()?;
    // Kept readable after compositing so the frame can be read back for
    // verification; the cost is one extra buffer swap.
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(
        &opts,
        &JsValue::from_str("preserveDrawingBuffer"),
        &JsValue::TRUE,
    )?;
    let ctx = canvas
        .get_context_with_context_options("webgl2", &opts)?
        .ok_or("webgl2 unavailable")?
        .dyn_into::<Gl>()?;
    Ok((canvas, ctx))
}

/// Compile a shader and surface the driver's log if it fails.
fn shader(gl: &Gl, kind: u32, src: &str) -> Result<WebGlShader, JsValue> {
    let sh = gl.create_shader(kind).ok_or("create_shader failed")?;
    gl.shader_source(&sh, src);
    gl.compile_shader(&sh);
    let ok = gl
        .get_shader_parameter(&sh, COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false);
    if !ok {
        return Err(JsValue::from_str(&format!(
            "shader compile error: {}",
            gl.get_shader_info_log(&sh).unwrap_or_default()
        )));
    }
    Ok(sh)
}

/// Build a linked program. `attrs` are bound to locations 0..n by index, which
/// removes any need for `get_attrib_location` at draw time.
pub fn program(gl: &Gl, vs: &str, fs: &str, attrs: &[&str]) -> Result<WebGlProgram, JsValue> {
    let v = shader(gl, VERTEX_SHADER, vs)?;
    let f = shader(gl, FRAGMENT_SHADER, fs)?;
    let p = gl.create_program().ok_or("create_program failed")?;
    gl.attach_shader(&p, &v);
    gl.attach_shader(&p, &f);
    for (i, name) in attrs.iter().enumerate() {
        gl.bind_attrib_location(&p, i as u32, name);
    }
    gl.link_program(&p);
    let ok = gl
        .get_program_parameter(&p, LINK_STATUS)
        .as_bool()
        .unwrap_or(false);
    if !ok {
        return Err(JsValue::from_str(&format!(
            "link error: {}",
            gl.get_program_info_log(&p).unwrap_or_default()
        )));
    }
    gl.detach_shader(&p, &v);
    gl.detach_shader(&p, &f);
    gl.delete_shader(Some(&v));
    gl.delete_shader(Some(&f));
    Ok(p)
}

/// Upload a float array with no copy where the engine allows it.
pub fn upload_f32(gl: &Gl, target: u32, data: &[f32], usage: u32) {
    let view = unsafe { js_sys::Float32Array::view(data) };
    gl.buffer_data_with_array_buffer_view(target, &view, usage);
}

/// Upload a u16 index array.
pub fn upload_u16(gl: &Gl, target: u32, data: &[u16], usage: u32) {
    let view = unsafe { js_sys::Uint16Array::view(data) };
    gl.buffer_data_with_array_buffer_view(target, &view, usage);
}

/// Size the drawing buffer to the canvas CSS box times a capped device ratio.
pub fn fit(canvas: &HtmlCanvasElement, gl: &Gl, cap: f64) -> (i32, i32) {
    let rect = canvas.get_bounding_client_rect();
    let dpr = web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0)
        .min(cap);
    let w = ((rect.width() * dpr).round() as i32).max(1);
    let h = ((rect.height() * dpr).round() as i32).max(1);
    if canvas.width() != w as u32 || canvas.height() != h as u32 {
        canvas.set_width(w as u32);
        canvas.set_height(h as u32);
    }
    gl.viewport(0, 0, w, h);
    (w, h)
}
