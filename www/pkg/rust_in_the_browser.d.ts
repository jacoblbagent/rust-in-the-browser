/* tslint:disable */
/* eslint-disable */

export class Galaxy {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add `count` new bodies on steep infall orbits.
     */
    add_infall(count: number): void;
    count(): number;
    /**
     * One physics step plus one draw. `dt` is seconds.
     */
    frame(dt: number): void;
    /**
     * Milliseconds spent in the last integration step.
     */
    last_step_ms(): number;
    constructor(canvas_id: string, count: number);
    /**
     * Rebuild the disc. `count` cannot exceed the capacity given at construction.
     */
    populate(count: number): void;
    /**
     * Flat `x, y, z` triples of the live body positions. The verification
     * harness reads this to check the disc's radial profile directly, rather
     * than inferring it from pixels.
     */
    positions(): Float32Array;
}

export class Knot {
    free(): void;
    [Symbol.dispose](): void;
    drag(dx: number, dy: number): void;
    frame(dt: number): void;
    constructor(canvas_id: string);
    /**
     * Stop the auto-spin once the visitor drags the model themselves.
     */
    set_auto(on: boolean): void;
    triangles(): number;
    vertices(): number;
}

export class Life {
    free(): void;
    [Symbol.dispose](): void;
    cells(): number;
    generation(): number;
    height(): number;
    constructor(w: number, h: number, seed: number);
    pixels(): Uint8ClampedArray;
    population(): number;
    /**
     * Paint heat into the RGBA buffer.
     */
    render(): void;
    reseed(): void;
    /**
     * One generation. Eight neighbours per cell, wrapped edges.
     */
    step(): void;
    width(): number;
}

export class Mandelbrot {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Drift the palette so the still image keeps moving.
     */
    advance(dt: number): void;
    byte_len(): number;
    center_x(): number;
    center_y(): number;
    /**
     * Slow brightness drift, derived from the phase with a triangle wave so
     * no transcendental ends up in the per-pixel path. Exported so the
     * JavaScript baseline runs on the same exposure.
     */
    gain(): number;
    height(): number;
    max_iter(): number;
    constructor(w: number, h: number);
    /**
     * Zero-copy view of the RGBA buffer. wasm-bindgen hands this back as a
     * plain byte array, so the page wraps it as a clamped array before it
     * becomes an ImageData.
     */
    pixels(): Uint8ClampedArray;
    /**
     * One pixel at a time. This is the function the benchmark times.
     */
    render(): void;
    /**
     * The same image, two pixels per instruction. Falls back to the scalar
     * kernel on a target without `simd128`.
     */
    render_wide(): void;
    /**
     * Read back the live window, so the JavaScript baseline always runs on the
     * same rectangle the Rust kernel is using.
     */
    scale(): number;
    set_center(cx: number, cy: number): void;
    set_max_iter(n: number): void;
    /**
     * Pin the palette, so the benchmark compares a fixed image.
     */
    set_phase(p: number): void;
    set_zoom(zoom: number): void;
    width(): number;
}

/**
 * A hand-rolled timing loop, exported so the page can ask the module to run
 * the fractal benchmark on the Rust side of the wall. `wide` selects the
 * two-lane kernel.
 */
export function bench_mandelbrot(iters: number, max_iter: number, w: number, h: number, wide: boolean): number;

export function build_epoch(): number;

/**
 * Target feature detection, reported on the page rather than assumed.
 */
export function has_simd(): boolean;

/**
 * Trap panics into the console instead of a bare "unreachable" error.
 */
export function start(): void;

/**
 * Build facts, captured at compile time by `build.rs`.
 */
export function toolchain(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_galaxy_free: (a: number, b: number) => void;
    readonly __wbg_knot_free: (a: number, b: number) => void;
    readonly __wbg_life_free: (a: number, b: number) => void;
    readonly __wbg_mandelbrot_free: (a: number, b: number) => void;
    readonly bench_mandelbrot: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly build_epoch: () => number;
    readonly galaxy_add_infall: (a: number, b: number) => void;
    readonly galaxy_count: (a: number) => number;
    readonly galaxy_frame: (a: number, b: number) => void;
    readonly galaxy_last_step_ms: (a: number) => number;
    readonly galaxy_new: (a: number, b: number, c: number) => [number, number, number];
    readonly galaxy_populate: (a: number, b: number) => void;
    readonly galaxy_positions: (a: number) => [number, number];
    readonly has_simd: () => number;
    readonly knot_drag: (a: number, b: number, c: number) => void;
    readonly knot_frame: (a: number, b: number) => void;
    readonly knot_new: (a: number, b: number) => [number, number, number];
    readonly knot_set_auto: (a: number, b: number) => void;
    readonly knot_triangles: (a: number) => number;
    readonly knot_vertices: (a: number) => number;
    readonly life_cells: (a: number) => number;
    readonly life_generation: (a: number) => number;
    readonly life_height: (a: number) => number;
    readonly life_new: (a: number, b: number, c: number) => number;
    readonly life_pixels: (a: number) => any;
    readonly life_population: (a: number) => number;
    readonly life_render: (a: number) => void;
    readonly life_reseed: (a: number) => void;
    readonly life_step: (a: number) => void;
    readonly life_width: (a: number) => number;
    readonly mandelbrot_advance: (a: number, b: number) => void;
    readonly mandelbrot_byte_len: (a: number) => number;
    readonly mandelbrot_center_x: (a: number) => number;
    readonly mandelbrot_center_y: (a: number) => number;
    readonly mandelbrot_gain: (a: number) => number;
    readonly mandelbrot_height: (a: number) => number;
    readonly mandelbrot_max_iter: (a: number) => number;
    readonly mandelbrot_new: (a: number, b: number) => number;
    readonly mandelbrot_pixels: (a: number) => any;
    readonly mandelbrot_render: (a: number) => void;
    readonly mandelbrot_render_wide: (a: number) => void;
    readonly mandelbrot_scale: (a: number) => number;
    readonly mandelbrot_set_center: (a: number, b: number, c: number) => void;
    readonly mandelbrot_set_max_iter: (a: number, b: number) => void;
    readonly mandelbrot_set_phase: (a: number, b: number) => void;
    readonly mandelbrot_set_zoom: (a: number, b: number) => void;
    readonly mandelbrot_width: (a: number) => number;
    readonly start: () => void;
    readonly toolchain: () => [number, number];
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
