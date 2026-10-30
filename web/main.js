// Runs the shared Rust game (trail.wasm) in the browser: keys in, pixels and beeps out.

const MOD_KEY = "trail.mod";
const SCORES_KEY = "trail.scores";
const SLOT_KEY = (i) => "trail.slot" + (i + 1);
const SLOTS = 5;

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
// On the play page the game owns the keyboard. Embedded in a bigger page
// (<canvas data-embedded>), it only listens while the canvas has focus.
const embedded = canvas.hasAttribute("data-embedded");
const focusGame = () => canvas.focus({ preventScroll: true });

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
    // Sound is allowed now: play the title song if we're on the title.
    if (audio && wasm) wasm.trail_title_song();
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

const held = new Set();

function send(code, down) {
  if (down) held.add(code); else held.delete(code);
  wasm.trail_key(code, down ? 1 : 0);
}

function onKey(e, down) {
  if (!wasm || paused) return;
  if (embedded && document.activeElement !== canvas) return;
  if (modPanel.contains(document.activeElement) && document.activeElement !== canvas) return;
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const code = keyCode(e);
  if (code === null) return;
  e.preventDefault();
  if (down) wakeAudio();
  if (down && e.repeat && code !== 8) return;
  send(code, down);
}

window.addEventListener("keydown", (e) => onKey(e, true));
window.addEventListener("keyup", (e) => onKey(e, false));
canvas.addEventListener("pointerdown", () => { focusGame(); wakeAudio(); });
// Let go of held keys when the game loses focus, so the hunter doesn't keep walking.
canvas.addEventListener("blur", () => { for (const code of [...held]) send(code, false); });

// On-screen keyboard for phones and tablets, laid out like the Cardputer's.
// Each key is [label, key, class]; SHIFT makes the next letter a capital.
const PAD = [
  [..."1234567890"].map((c) => [c, c]).concat([["DEL", "Backspace", "wide"]]),
  [..."qwertyuiop"].map((c) => [c, c]),
  [..."asdfghjkl"].map((c) => [c, c]).concat([["ENTER", "Enter", "wide"]]),
  [["SHIFT", "Shift", "wide"]].concat([..."zxcvbnm,."].map((c) => [c, c])),
];
// The bottom: ESC, MENU and SPACE on the left, the arrows as an upside-down T on the right.
const PAD_LEFT = [[["ESC", "Escape"], ["MENU", "`"]], [["SPACE", " "]]];
const PAD_ARROWS = [null, ["\u25b2", "ArrowUp", "arrow"], null,
  ["\u25c0", "ArrowLeft", "arrow"], ["\u25bc", "ArrowDown", "arrow"], ["\u25b6", "ArrowRight", "arrow"]];
// Key colors match the game's (KEY_COLORS in gfx.rs): each number its own color,
// ENTER and Y green for go, ESC and N red for stop, SPACE white.
const KEY_COLORS = ["#5aa0ff", "#fadc46", "#ff78be", "#b982ff", "#3cd2c8", "#fa9632", "#c88c50", "#ebebeb", "#a0a0a0"];
const GO = "#50d25a", STOP = "#f05046", SPACE = "#ebebeb";
const keyColor = (k) =>
  /^[1-9]$/.test(k) ? KEY_COLORS[k - 1]
    : k === "Enter" || k === "y" ? GO
    : k === "Escape" || k === "n" ? STOP
    : k === " " ? SPACE : null;
const padEl = document.getElementById("pad");
let shift = false;
const letters = [];
function setShift(on) {
  shift = on;
  for (const b of letters) b.textContent = on ? b.dataset.key.toUpperCase() : b.dataset.key;
  padEl.querySelector("[data-key=Shift]").classList.toggle("on", on);
}
function div(parent, cls) {
  const d = parent.appendChild(document.createElement("div"));
  d.className = cls;
  return d;
}
function padKey(parent, [label, k, cls]) {
  const btn = parent.appendChild(document.createElement("button"));
  btn.textContent = label;
  btn.dataset.key = k;
  if (cls) btn.className = cls;
  const color = keyColor(k);
  if (color) {
    btn.style.background = color;
    btn.style.color = "#000";
    btn.style.borderColor = color;
  }
  if (k === "`") btn.title = "Your menu when playing together";
  if (/^[a-z]$/.test(k)) letters.push(btn);
  let code = null;
  btn.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    if (!wasm || paused) return;
    wakeAudio();
    focusGame();
    if (k === "Shift") return setShift(!shift);
    const key = shift && letters.includes(btn) ? k.toUpperCase() : k;
    code = key in SPECIAL ? SPECIAL[key] : key.codePointAt(0);
    send(code, true);
    if (shift) setShift(false);
  });
  for (const ev of ["pointerup", "pointerleave", "pointercancel"]) {
    btn.addEventListener(ev, () => {
      if (wasm && code !== null && held.has(code)) send(code, false);
      code = null;
    });
  }
}
for (const keys of PAD) {
  const row = div(padEl, "row");
  for (const key of keys) padKey(row, key);
}
// Touch screens always show the keys; with a mouse they're a button away.
const keysBtn = document.getElementById("keysbtn");
function showKeys(on) {
  padEl.classList.toggle("show", on);
  keysBtn.textContent = on ? "Hide keys" : "Show keys";
  store.set("trail.keys", on ? "on" : "off");
}
keysBtn.onclick = () => { showKeys(!padEl.classList.contains("show")); focusGame(); };
showKeys(store.get("trail.keys") === "on");
const bottom = div(padEl, "row bottom");
const left = div(bottom, "left");
for (const keys of PAD_LEFT) {
  const row = div(left, "row");
  for (const key of keys) padKey(row, key);
}
const arrows = div(bottom, "arrows");
for (const key of PAD_ARROWS) {
  if (key) padKey(arrows, key);
  else div(arrows, "gap");
}

// ---- main loop ----
let last = performance.now();
let frames = 0, fpsTime = last;
const fpsEl = document.getElementById("fps");

// While the restart question is up the game stands still.
let paused = false;

function frame(now) {
  const dt = Math.min(250, now - last);
  last = now;
  if (paused) return requestAnimationFrame(frame);
  wasm.trail_tick(Math.round(dt));
  playBeeps();
  sendNet();
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
  // Saved trips go to browser storage as soon as the game makes them.
  for (let kind = wasm.trail_next_save(); kind >= 0; kind = wasm.trail_next_save()) {
    if (kind === 100) store.set(SCORES_KEY, readText());
    else if (kind < SLOTS) store.set(SLOT_KEY(kind), readText());
  }
  requestAnimationFrame(frame);
}

// ---- playing together ----
// Only when the page came from a Westward Trail relay (it answers relay.json).
// GitHub Pages and other plain hosts don't, so nothing connects there.
let ws = null;
let online = false;

function connectRelay() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  try {
    ws = new WebSocket(`${proto}//${location.host}/ws`);
  } catch {
    return;
  }
  ws.onopen = () => { online = true; wasm.trail_set_online(1); notify("Connected: play together!"); };
  ws.onmessage = (e) => {
    if (typeof e.data !== "string") return;
    writeText(e.data);
    wasm.trail_net_in();
  };
  ws.onclose = () => {
    if (online) notify("Lost the connection");
    online = false;
    wasm.trail_set_online(0);
    setTimeout(connectRelay, 3000);
  };
}

async function maybeRelay() {
  try {
    const r = await fetch("relay.json", { cache: "no-store" });
    if (r.ok && (await r.json()).relay) connectRelay();
  } catch { /* not served by a relay */ }
}

function sendNet() {
  while (wasm.trail_net_next()) {
    const line = readText();
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(line);
  }
}

// ---- saved trips & screenshots ----
function loadSlots() {
  for (let i = 0; i < SLOTS; i++) {
    const text = store.get(SLOT_KEY(i));
    if (text) {
      writeText(text);
      wasm.trail_import_slot(i);
    }
  }
}

function notify(text) {
  writeText(text);
  wasm.trail_notify();
}

// A crisp PNG of the screen, 4x bigger so the pixels stay sharp.
function screenshot() {
  const scale = 4;
  const big = document.createElement("canvas");
  big.width = canvas.width * scale;
  big.height = canvas.height * scale;
  const g = big.getContext("2d");
  g.imageSmoothingEnabled = false;
  g.drawImage(canvas, 0, 0, big.width, big.height);
  const d = new Date();
  const pad = (n) => String(n).padStart(2, "0");
  const name = `westward-trail-${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.png`;
  big.toBlob((blob) => {
    if (!blob) return;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
    notify("Screenshot saved!");
  }, "image/png");
}

const shotBtn = document.getElementById("shot");
if (shotBtn) shotBtn.onclick = () => { screenshot(); focusGame(); };

// ---- mod editor ----
function setStatus(text, ok) {
  statusEl.textContent = text;
  statusEl.className = ok ? "ok" : "bad";
}

function applyMod(text, quiet) {
  writeText(text);
  const ok = wasm.trail_load_mod() === 1;
  if (!quiet || !ok) setStatus(readText(), ok);
  if (ok) loadSlots(); // a new trail starts a new game; give it the saved trips again
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
    focusGame();
  }
};

document.getElementById("reset").onclick = () => {
  modText.value = builtInData();
  store.del(MOD_KEY);
  wasm.trail_init(Date.now() >>> 0);
  loadSlots();
  setStatus("Back to the original trail.", true);
};

// Restart asks first, with the game paused, since the button sits near the keys.
const ask = document.createElement("dialog");
ask.className = "ask";
ask.innerHTML = `<p>Restart the game?</p><p class="small">Your trip so far is lost. Saved trips are kept.</p>
  <form method="dialog"><button value="no" autofocus>Keep playing</button> <button value="yes">Restart</button></form>`;
document.body.appendChild(ask);
ask.addEventListener("close", () => {
  paused = false;
  if (ask.returnValue === "yes") {
    const mod = store.get(MOD_KEY);
    wasm.trail_init(Date.now() >>> 0);
    if (mod) applyMod(mod, true);
    else loadSlots();
  }
  focusGame();
});
document.getElementById("restart").onclick = () => {
  for (const code of [...held]) send(code, false);
  paused = true;
  ask.returnValue = "no";
  ask.showModal();
};

const soundBtn = document.getElementById("sound");
function showSound() { soundBtn.textContent = "Sound: " + (soundOn ? "on" : "off"); }
soundBtn.onclick = () => {
  soundOn = !soundOn;
  store.set("trail.sound", soundOn ? "on" : "off");
  showSound();
  wakeAudio();
  focusGame();
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
  loadSlots();
  const mod = store.get(MOD_KEY);
  if (mod && !applyMod(mod, true)) {
    modPanel.classList.add("open");
    modText.value = mod;
  }
  image = ctx.createImageData(wasm.trail_width(), wasm.trail_height());
  if (!embedded) focusGame();
  requestAnimationFrame(frame);
  maybeRelay();
})().catch((err) => {
  ctx.fillStyle = "#fff";
  ctx.font = "10px monospace";
  ctx.fillText("Could not load trail.wasm", 10, 20);
  ctx.fillText("Run ./web/build.sh, then serve web/", 10, 34);
  console.error(err);
});
