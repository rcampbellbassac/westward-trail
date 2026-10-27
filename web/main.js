// Runs the shared Rust game (trail.wasm) in the browser: keys in, pixels and beeps out.

const MOD_KEY = "trail.mod";
const SCORES_KEY = "trail.scores";

const store = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
  del(k) { try { localStorage.removeItem(k); } catch { /* private mode */ } },
};

const canvas = document.getElementById("screen");
const ctx = canvas.getContext("2d");
const modPanel = document.getElementById("mod");
const modText = document.getElementById("modtext");
const statusEl = document.getElementById("status");

let wasm, image;
const enc = new TextEncoder();
const dec = new TextDecoder();

function writeText(s) {
  const bytes = enc.encode(s);
  const ptr = wasm.trail_text_in(bytes.length);
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
}

function readText() {
  const ptr = wasm.trail_text_out_ptr();
  const len = wasm.trail_text_out_len();
  return dec.decode(new Uint8Array(wasm.memory.buffer, ptr, len));
}

// ---- sound: a tiny square-wave "PC speaker" ----
let audio = null;
let soundOn = store.get("trail.sound") !== "off";
let audioTime = 0;

function playBeeps() {
  const n = wasm.trail_beep_count();
  if (n === 0) return;
  if (soundOn && audio) {
    let t = Math.max(audio.currentTime, audioTime);
    for (let i = 0; i < n; i++) {
      const packed = wasm.trail_beep(i);
      const hz = packed >>> 16;
      const ms = packed & 0xffff;
      if (hz > 0) {
        const osc = audio.createOscillator();
        const gain = audio.createGain();
        osc.type = "square";
        osc.frequency.value = hz;
        gain.gain.value = 0.06;
        osc.connect(gain).connect(audio.destination);
        osc.start(t);
        osc.stop(t + ms / 1000);
      }
      t += ms / 1000;
    }
    audioTime = Math.min(t, audio.currentTime + 2);
  }
  wasm.trail_beeps_clear();
}

function wakeAudio() {
  if (!audio) {
    try { audio = new AudioContext(); } catch { audio = null; }
  } else if (audio.state === "suspended") {
    audio.resume();
  }
}

// ---- keyboard ----
const SPECIAL = {
  Enter: 13, Backspace: 8, Escape: 27,
  ArrowUp: 0xe000, ArrowDown: 0xe001, ArrowLeft: 0xe002, ArrowRight: 0xe003,
};

function keyCode(e) {
  if (e.key in SPECIAL) return SPECIAL[e.key];
  if (e.key.length === 1) return e.key.codePointAt(0);
  return null;
}

function onKey(e, down) {
  if (modPanel.contains(document.activeElement) && document.activeElement !== canvas) return;
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const code = keyCode(e);
  if (code === null) return;
  e.preventDefault();
  if (down) wakeAudio();
  if (down && e.repeat && code !== 8) return;
  wasm.trail_key(code, down ? 1 : 0);
}

window.addEventListener("keydown", (e) => onKey(e, true));
window.addEventListener("keyup", (e) => onKey(e, false));
canvas.addEventListener("pointerdown", () => { canvas.focus(); wakeAudio(); });

// ---- main loop ----
let last = performance.now();
let frames = 0, fpsTime = last;
const fpsEl = document.getElementById("fps");

function frame(now) {
  const dt = Math.min(250, now - last);
  last = now;
  wasm.trail_tick(Math.round(dt));
  playBeeps();
  const ptr = wasm.trail_render();
  image.data.set(new Uint8Array(wasm.memory.buffer, ptr, image.data.length));
  ctx.putImageData(image, 0, 0);
  frames++;
  if (now - fpsTime > 1000) {
    fpsEl.textContent = frames + " fps";
    frames = 0;
    fpsTime = now;
    store.set(SCORES_KEY, (wasm.trail_export_scores(), readText()));
  }
  requestAnimationFrame(frame);
}

// ---- mod editor ----
function setStatus(text, ok) {
  statusEl.textContent = text;
  statusEl.className = ok ? "ok" : "bad";
}

function applyMod(text, quiet) {
  writeText(text);
  const ok = wasm.trail_load_mod() === 1;
  if (!quiet || !ok) setStatus(readText(), ok);
  return ok;
}

function builtInData() {
  wasm.trail_default_data();
  return readText();
}

document.getElementById("modbtn").onclick = () => {
  modPanel.classList.toggle("open");
  if (modPanel.classList.contains("open") && !modText.value) {
    modText.value = store.get(MOD_KEY) || builtInData();
  }
};

document.getElementById("apply").onclick = () => {
  if (applyMod(modText.value)) {
    store.set(MOD_KEY, modText.value);
    canvas.focus();
  }
};

document.getElementById("reset").onclick = () => {
  modText.value = builtInData();
  store.del(MOD_KEY);
  wasm.trail_init(Date.now() >>> 0);
  setStatus("Back to the original trail.", true);
};

document.getElementById("restart").onclick = () => {
  const mod = store.get(MOD_KEY);
  wasm.trail_init(Date.now() >>> 0);
  if (mod) applyMod(mod, true);
  canvas.focus();
};

const soundBtn = document.getElementById("sound");
function showSound() { soundBtn.textContent = "Sound: " + (soundOn ? "on" : "off"); }
soundBtn.onclick = () => {
  soundOn = !soundOn;
  store.set("trail.sound", soundOn ? "on" : "off");
  showSound();
  wakeAudio();
  canvas.focus();
};
showSound();

// ---- start ----
(async () => {
  const resp = await fetch("trail.wasm");
  const { instance } = await WebAssembly.instantiate(await resp.arrayBuffer(), {});
  wasm = instance.exports;
  wasm.trail_init(Date.now() >>> 0);
  const scores = store.get(SCORES_KEY);
  if (scores) {
    writeText(scores);
    wasm.trail_import_scores();
  }
  const mod = store.get(MOD_KEY);
  if (mod && !applyMod(mod, true)) {
    modPanel.classList.add("open");
    modText.value = mod;
  }
  image = ctx.createImageData(wasm.trail_width(), wasm.trail_height());
  canvas.focus();
  requestAnimationFrame(frame);
})().catch((err) => {
  ctx.fillStyle = "#fff";
  ctx.font = "10px monospace";
  ctx.fillText("Could not load trail.wasm", 10, 20);
  ctx.fillText("Run ./web/build.sh, then serve web/", 10, 34);
  console.error(err);
});
