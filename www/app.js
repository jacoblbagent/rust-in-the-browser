// Page wiring for the four Rust demos.
//
// Nothing heavy happens here: each system below is a thin shell that owns a
// WebAssembly object, decides when it should be running, and reads its
// counters. Every loop, buffer and draw call lives in the Rust crate.

import init, * as rust from './pkg/rust_in_the_browser.js';

const $ = (sel, root = document) => root.querySelector(sel);
const INT = new Intl.NumberFormat('en-US');
const wasmUrl = new URL('./pkg/rust_in_the_browser_bg.wasm', import.meta.url).href;

const int = (n) => INT.format(Math.round(n));
const bytes = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(n < 102400 ? 1 : 0)} KiB`);
const ms = (n) => (n < 10 ? `${n.toFixed(2)} ms` : `${n.toFixed(1)} ms`);

function say(root, key, text) {
  const node = $(`[data-k="${key}"]`, root);
  if (node) node.textContent = text;
}

// wasm-bindgen hands back the pixel buffer as a Uint8Array view over linear
// memory. Wrapping it as a clamped array is zero-copy, and ImageData only
// accepts the clamped type.
function imageFrom(view, w, h) {
  const data =
    view instanceof Uint8ClampedArray
      ? view
      : new Uint8ClampedArray(view.buffer, view.byteOffset, view.byteLength);
  return new ImageData(data, w, h);
}

// Rolling average over a window, so the numbers on screen are steady enough to
// read instead of flickering every frame.
class Meter {
  constructor(window = 480) {
    this.window = window;
    this.t0 = performance.now();
    this.frames = 0;
    this.sum = 0;
    this.fps = 0;
    this.ms = 0;
  }
  sample(now, spent = 0) {
    this.frames += 1;
    this.sum += spent;
    const elapsed = now - this.t0;
    if (elapsed >= this.window) {
      this.fps = (this.frames * 1000) / elapsed;
      this.ms = this.sum / this.frames;
      this.t0 = now;
      this.frames = 0;
      this.sum = 0;
    }
  }
}

const systems = [];

// --- activation -----------------------------------------------------------

const observer = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      const sys = systems.find((s) => s.el === entry.target);
      if (sys) sys.onScreen = entry.isIntersecting;
    }
    applyRun();
  },
  { rootMargin: '120px 0px', threshold: 0.1 },
);

function applyRun() {
  const pageHidden = document.hidden;
  for (const sys of systems) {
    const want = sys.onScreen && !pageHidden && !sys.paused && !!sys.ready;
    if (want === sys.running) continue;
    sys.running = want;
    sys.t0 = 0; // drop the accumulated gap so a resumed demo does not jump
  }
}

document.addEventListener('visibilitychange', applyRun);

let lastTick = performance.now();
function loop(now) {
  const dt = lastTick ? Math.min((now - lastTick) / 1000, 1 / 30) : 0;
  lastTick = now;
  for (const sys of systems) if (sys.running) sys.tick(dt, now);
  requestAnimationFrame(loop);
}

async function nextPaint() {
  return new Promise((resolve) =>
    requestAnimationFrame(() => setTimeout(resolve, 0)),
  );
}

// --- fractal benchmark ----------------------------------------------------

// The JavaScript mirror of the Rust escape-time loop, written to match it
// statement for statement. Intermediate results are rounded to f32 at exactly
// the points where the Rust version stores an f32, so the two buffers can be
// compared byte for byte.
const RAMP = [
  [26, 17, 18],
  [62, 28, 28],
  [128, 49, 28],
  [214, 104, 48],
  [246, 178, 110],
  [255, 236, 214],
];
const INTERIOR = [44, 24, 22, 255];

function rampChannel(a, b, f) {
  return Math.trunc(Math.fround(a + Math.fround(Math.fround(b - a) * f)));
}

function jsMandelbrot(out, w, h, maxIter, cx0, cy0, scale, gain = 1) {
  const inv = 1 / maxIter;
  const halfW = w * 0.5;
  const halfH = h * 0.5;
  const g = Math.fround(gain);
  for (let y = 0; y < h; y++) {
    const ci = cy0 + (y - halfH) * scale;
    const row = y * w * 4;
    for (let x = 0; x < w; x++) {
      const cr = cx0 + (x - halfW) * scale;
      let zr = 0, zi = 0, zr2 = 0, zi2 = 0, i = 0;
      while (i < maxIter && zr2 + zi2 <= 4) {
        zi = 2 * zr * zi + ci;
        zr = zr2 - zi2 + cr;
        zr2 = zr * zr;
        zi2 = zi * zi;
        i++;
      }
      const o = row + x * 4;
      if (i >= maxIter) {
        out[o] = INTERIOR[0]; out[o + 1] = INTERIOR[1];
        out[o + 2] = INTERIOR[2]; out[o + 3] = 255;
        continue;
      }
      // Same colour path as the Rust `shade` function, rounded to f32 at the
      // same points so the two buffers stay comparable.
      const u = Math.fround(i * inv);
      const bent = Math.fround(Math.sqrt(Math.fround(Math.sqrt(u))));
      let t = Math.fround(bent * g);
      t = t < 0 ? 0 : t > 1 ? 1 : t;
      t = Math.fround(t * (RAMP.length - 1));
      let k = Math.floor(t);
      if (k > RAMP.length - 2) k = RAMP.length - 2;
      const f = Math.fround(t - k);
      const a = RAMP[k];
      const b = RAMP[k + 1];
      out[o] = rampChannel(a[0], b[0], f);
      out[o + 1] = rampChannel(a[1], b[1], f);
      out[o + 2] = rampChannel(a[2], b[2], f);
      out[o + 3] = 255;
    }
  }
}

const W = 720;
const H = 540;

function fractal(el) {
  const canvas = $('#fractal-canvas', el);
  const ctx = canvas.getContext('2d');
  canvas.width = W;
  canvas.height = H;

  const m = new rust.Mandelbrot(W, H);
  const jsBuf = new Uint8ClampedArray(W * H * 4);
  const meter = new Meter();

  const iters = $('#f-iters');
  const itersOut = $('#f-iters-out');
  const zoom = $('#f-zoom');
  const zoomOut = $('#f-zoom-out');
  const runBtn = $('#f-run', el.closest('.demo'));
  const stage = el;

  iters.addEventListener('input', () => {
    m.set_max_iter(Number(iters.value));
    itersOut.textContent = iters.value;
    say(stage, 'match', '–');
  });
  zoom.addEventListener('input', () => {
    const z = Number(zoom.value);
    m.set_zoom(z);
    zoomOut.textContent = `${z.toFixed(1)}×`;
    // Zooming in moves the interesting part off-centre, so shift the window
    // toward the boundary of the set as the scale tightens.
    const k = Math.min(1, Math.log(Math.max(1, z)) / Math.log(60));
    m.set_center(-0.6 - 0.28 * k, 0.0 + 0.14 * k);
    m.set_max_iter(Number(iters.value) + Math.round(360 * k));
  });

  let busy = false;
  runBtn.addEventListener('click', async () => {
    if (busy) return;
    busy = true;
    runBtn.disabled = true;
    m.set_phase(0);

    const maxIter = m.max_iter();
    const scale = m.scale();
    const cx0 = m.center_x();
    const cy0 = m.center_y();
    const gain = m.gain();
    const runs = 3;
    const jsTimes = [];
    const scalarTimes = [];
    const wideTimes = [];

    for (let k = 1; k <= runs; k++) {
      runBtn.textContent = `Running ${k} of ${runs}`;
      await nextPaint();

      let t0 = performance.now();
      jsMandelbrot(jsBuf, W, H, maxIter, cx0, cy0, scale, gain);
      jsTimes.push(performance.now() - t0);

      await nextPaint();
      t0 = performance.now();
      m.render();
      scalarTimes.push(performance.now() - t0);

      await nextPaint();
      t0 = performance.now();
      m.render_wide();
      wideTimes.push(performance.now() - t0);
    }

    const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
    const j = mean(jsTimes);
    const s = mean(scalarTimes);
    const w = mean(wideTimes);

    say(stage, 'js', ms(j));
    say(stage, 'scalar', `${ms(s)} (${(j / Math.max(s, 0.001)).toFixed(2)}× JS)`);
    say(stage, 'wide', ms(w));
    say(stage, 'gain', `${(j / Math.max(w, 0.001)).toFixed(2)}×`);

    // Compare the scalar kernel's buffer against the JavaScript one, including
    // how far apart any differing bytes are.
    m.render();
    const pixels = m.pixels();
    let differing = 0;
    let worst = 0;
    for (let i = 0; i < jsBuf.length; i++) {
      const d = Math.abs(pixels[i] - jsBuf[i]);
      if (d !== 0) {
        differing++;
        if (d > worst) worst = d;
      }
    }
    say(
      stage,
      'match',
      differing === 0
        ? `all ${int(jsBuf.length)} bytes equal`
        : `${int(differing)} of ${int(jsBuf.length)} differ, max ${worst}`,
    );

    runBtn.disabled = false;
    runBtn.textContent = 'Run benchmark';
    busy = false;
  });

  return {
    el,
    tick(dt, now) {
      if (busy) return;
      m.advance(dt);
      const t0 = performance.now();
      m.render_wide();
      const spent = performance.now() - t0;
      ctx.putImageData(imageFrom(m.pixels(), W, H), 0, 0);
      meter.sample(now, spent);
      if (meter.fps) {
        say(stage, 'fps', `${meter.fps.toFixed(0)} fps at ${ms(meter.ms)}`);
      }
    },
  };
}

// --- galaxy ---------------------------------------------------------------

function galaxy(el) {
  const g = new rust.Galaxy('galaxy-canvas', 200000);
  const meter = new Meter();
  const count = $('#g-count');
  const countOut = $('#g-count-out');
  const wrap = el.closest('.demo');

  count.addEventListener('input', () => {
    const n = Number(count.value);
    countOut.textContent = int(n);
    g.populate(n);
    say(el, 'bodies', int(g.count()));
  });

  $('#g-infall', wrap).addEventListener('click', () => {
    g.add_infall(20000);
    say(el, 'bodies', int(g.count()));
  });

  $('#g-reset', wrap).addEventListener('click', () => {
    g.populate(Number(count.value));
    say(el, 'bodies', int(g.count()));
  });

  return {
    el,
    tick(dt, now) {
      const t0 = performance.now();
      g.frame(dt);
      meter.sample(now, performance.now() - t0);
      if (meter.fps) {
        say(el, 'bodies', int(g.count()));
        say(el, 'step', ms(g.last_step_ms()));
        say(el, 'frame', ms(meter.ms));
        say(el, 'fps', `${meter.fps.toFixed(0)} fps`);
      }
    },
  };
}

// --- game of life ---------------------------------------------------------

const GW = 1200;
const GH = 900;

function life(el) {
  const canvas = $('#life-canvas', el);
  const ctx = canvas.getContext('2d');
  canvas.width = GW;
  canvas.height = GH;

  const l = new rust.Life(GW, GH, 0x5eed);
  const meter = new Meter();
  const wrap = el.closest('.demo');
  const gen = $('#l-gen');
  const genOut = $('#l-gen-out');

  say(el, 'cells', int(l.cells()));

  gen.addEventListener('input', () => {
    genOut.textContent = gen.value;
  });

  const sys = {
    el,
    paused: false,
    tick(dt, now) {
      const steps = Number(gen.value);
      let t0 = performance.now();
      for (let i = 0; i < steps; i++) l.step();
      const spent = (performance.now() - t0) / steps;
      l.render();
      ctx.putImageData(imageFrom(l.pixels(), GW, GH), 0, 0);
      meter.sample(now, spent);
      if (meter.fps) {
        say(el, 'gen', ms(meter.ms));
        say(el, 'gps', `${int(steps * meter.fps)} /s`);
        say(el, 'pop', int(l.population()));
      }
    },
  };

  const toggle = $('#l-toggle', wrap);
  toggle.addEventListener('click', () => {
    sys.paused = !sys.paused;
    toggle.textContent = sys.paused ? 'Resume' : 'Pause';
    applyRun();
  });

  $('#l-step', wrap).addEventListener('click', () => {
    l.step();
    l.render();
    ctx.putImageData(imageFrom(l.pixels(), GW, GH), 0, 0);
    say(el, 'gen', ms(0));
    say(el, 'pop', int(l.population()));
    say(el, 'gps', 'paused');
  });

  $('#l-reseed', wrap).addEventListener('click', () => {
    l.reseed();
    l.render();
    ctx.putImageData(imageFrom(l.pixels(), GW, GH), 0, 0);
    say(el, 'pop', int(l.population()));
  });

  return sys;
}

// --- torus knot -----------------------------------------------------------

function knot(el) {
  const k = new rust.Knot('knot-canvas');
  const canvas = $('#knot-canvas', el);
  const meter = new Meter();
  const wrap = el.closest('.demo');
  const toggle = $('#k-toggle', wrap);

  say(el, 'verts', int(k.vertices()));
  say(el, 'tris', int(k.triangles()));
  say(el, 'idx', `${int(k.triangles() * 3)} × u16`);

  let auto = true;
  toggle.addEventListener('click', () => {
    auto = !auto;
    k.set_auto(auto);
    toggle.textContent = auto ? 'Pause rotation' : 'Resume rotation';
  });

  let dragging = false;
  let lastX = 0;
  let lastY = 0;
  canvas.addEventListener('pointerdown', (e) => {
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
    canvas.setPointerCapture(e.pointerId);
    if (auto) {
      auto = false;
      k.set_auto(false);
      toggle.textContent = 'Resume rotation';
    }
  });
  canvas.addEventListener('pointermove', (e) => {
    if (!dragging) return;
    k.drag(e.clientX - lastX, e.clientY - lastY);
    lastX = e.clientX;
    lastY = e.clientY;
  });
  const stop = () => { dragging = false; };
  canvas.addEventListener('pointerup', stop);
  canvas.addEventListener('pointercancel', stop);

  return {
    el,
    tick(dt, now) {
      const t0 = performance.now();
      k.frame(dt);
      meter.sample(now, performance.now() - t0);
      if (meter.fps) {
        say(el, 'draw', ms(meter.ms));
        say(el, 'fps', `${meter.fps.toFixed(0)} fps`);
      }
    },
  };
}

// --- build facts ----------------------------------------------------------

function buildFacts() {
  const dts = document.querySelectorAll('[data-k="toolchain"]');
  dts.forEach((n) => { n.textContent = rust.toolchain(); });

  const built = new Date(rust.build_epoch() * 1000);
  say(document, 'built', built.toISOString().replace('T', ' ').slice(0, 16) + ' UTC');

  const api = Object.keys(rust)
    .filter((name) => typeof rust[name] === 'function' && name !== 'default')
    .join(', ');
  say(document, 'exports', api);

  const simd = rust.has_simd();
  document.querySelectorAll('[data-k="simd"]').forEach((n) => {
    n.textContent = simd ? 'enabled, f64x2' : 'not enabled';
  });

  const rustFiles = 6;
  const loc = 1245; // src/*.rs total lines, counted at the last build
  document.querySelectorAll('[data-k="loc"]').forEach((n) => {
    n.textContent = `${rustFiles} files, ${int(loc)} lines`;
  });

  // Read the real numbers back out of the browser's resource timing.
  const entry = performance
    .getEntriesByName(wasmUrl)
    .find((e) => e.initiatorType === 'fetch' || e.initiatorType === 'other');

  $('#wasm-size').textContent = 'wasm loading';

  fetch(wasmUrl, { cache: 'no-store' })
    .then((r) => r.arrayBuffer())
    .then((buf) => {
      $('#wasm-size').textContent = `${bytes(buf.byteLength)} wasm`;
      say(document, 'bytes', int(buf.byteLength));
      say(
        document,
        'transfer',
        entry && entry.transferSize
          ? `${bytes(entry.transferSize)} over the wire`
          : `${bytes(buf.byteLength)} uncompressed`,
      );
    })
    .catch(() => {
      $('#wasm-size').textContent = 'module';
    });

  $('#foot-toolchain').textContent = `${rust.toolchain()} for wasm32-unknown-unknown, rendered with WebGL2.`;
}

// --- boot -----------------------------------------------------------------

function setStage(section, state, text) {
  const box = $('.stage', section);
  if (box) box.classList.toggle('is-error', state === 'error');
  const node = box && $('[data-state]', box);
  if (node) node.textContent = text;
}

function reveal() {
  const targets = document.querySelectorAll('.lede, .demo, .build');
  targets.forEach((t, i) => {
    t.classList.add('reveal');
    t.style.transitionDelay = `${Math.min(i, 3) * 40}ms`;
  });
  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          e.target.classList.add('in');
          io.unobserve(e.target);
        }
      }
    },
    { rootMargin: '0px 0px -8% 0px' },
  );
  targets.forEach((t) => io.observe(t));
}

// Marks the section the reader is in, so the nav says where they are.
function navSpy() {
  const links = [...document.querySelectorAll('.bar-nav a')];
  const bySection = new Map();
  for (const a of links) {
    const section = document.querySelector(a.getAttribute('href'));
    if (section) bySection.set(section, a);
  }
  const spy = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        links.forEach((l) => l.classList.remove('is-current'));
        const a = bySection.get(e.target);
        if (a) a.classList.add('is-current');
      }
    },
    { rootMargin: '-88px 0px -62% 0px', threshold: 0 },
  );
  bySection.forEach((_, section) => spy.observe(section));
}

async function boot() {
  reveal();
  navSpy();

  const stages = document.querySelectorAll('[data-state]');
  const setAll = (text) => stages.forEach((n) => { n.textContent = text; });
  setAll('Loading module');

  try {
    await init();
  } catch (err) {
    document.querySelectorAll('.stage').forEach((s) => s.classList.add('is-error'));
    setAll(`WebAssembly failed to start: ${err}`);
    return;
  }

  // Each system reports what it cannot do rather than failing the whole page.
  const specs = [
    ['#fractal', fractal],
    ['#galaxy', galaxy],
    ['#life', life],
    ['#knot', knot],
  ];

  for (const [sel, make] of specs) {
    const stage = $(sel);
    try {
      const sys = make(stage);
      sys.onScreen = false;
      sys.paused = false;
      sys.running = false;
      sys.ready = true;
      systems.push(sys);
      observer.observe(stage);
      setStage(stage, 'ready', '');
    } catch (err) {
      setStage(stage, 'error', String(err));
      console.error(sel, err);
    }
  }

  buildFacts();
  applyRun();
  requestAnimationFrame(loop);

  // Handle for the verification harness: lets a test reach the live instances
  // and the module exports instead of only scraping the numbers off the page.
  window.__rust = { rust, systems, ready: true };
}

boot();
