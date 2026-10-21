// Typing on touch screens (js/host.js `touchInstall` / `touchCover`) under Node with a fake DOM,
// no browser or Wasm build needed:
//
//   node apps/effectcraft-web/tests/touch.mjs
//
// eframe's text input lies over the text fields: a tap on it is the input's own (iOS opens the
// keyboard only for an input focused by the user's tap) and reaches egui once the input has the
// focus; a drag or a long press that began on it goes to egui, with the browser's handling
// prevented. Touches elsewhere are left to eframe. Exits non-zero on failure.
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
globalThis.self = globalThis;

const listeners = {};
const on = (type, f) => (listeners[type] ??= []).push(f);
globalThis.document = { activeElement: null, addEventListener: on };
globalThis.addEventListener = on;
globalThis.matchMedia = (q) => ({ matches: q === "(pointer: coarse)" });
globalThis.CSS = { supports: () => true };
let scrolled = 0;
globalThis.scrollTo = () => {
  scrolled++;
  globalThis.scrollX = globalThis.scrollY = 0;
};

const style = () => {
  const props = {};
  return new Proxy(props, {
    get: (o, k) => (k === "setProperty" ? (p, v) => (o[p] = v) : k === "removeProperty" ? (p) => delete o[p] : o[k]),
  });
};
const input = { tagName: "INPUT", style: style() };
const canvas = { tagName: "CANVAS", offsetLeft: 0, offsetTop: 0, nextElementSibling: input, getBoundingClientRect: () => ({ left: 10, top: 20 }) };

/// Dispatch a `type` event on `target`; returns whether it was prevented.
function fire(type, target, fields = {}) {
  let prevented = false;
  const e = { type, target, preventDefault: () => (prevented = true), stopPropagation() {}, ...fields };
  for (const f of listeners[type] ?? []) f(e);
  return prevented;
}
const finger = (id, x, y) => ({ identifier: id, clientX: x, clientY: y });
const touchEvent = (type, target, t, touches) => fire(type, target, { changedTouches: [t], touches: touches ?? (type === "touchstart" ? [t] : []) });

const host = await import(pathToFileURL(join(here, "../js/host.js")));
const taps = [];
const touches = [];
const HOLD = 60;
assert.equal(host.touchInstall(canvas, HOLD, (x, y) => taps.push([x, y]), (kind, id, x, y) => touches.push([kind, id, x, y])), true);

// A tap on the input: the browser focuses it, then egui gets the tap (at canvas coordinates).
touchEvent("touchstart", input, finger(1, 110, 220));
assert.equal(touchEvent("touchend", input, finger(1, 112, 221)), false, "the tap stays the input's: it focuses it");
assert.deepEqual(taps, [], "not before the input has the focus");
fire("focusin", input);
assert.deepEqual(taps.splice(0), [[100, 200]], "where the finger came down: on the field");
assert.deepEqual(touches, []);
// A tap that drifted (within the slop) off the field's edge still taps where it came down.
touchEvent("touchstart", input, finger(1, 110, 220));
touchEvent("touchmove", input, finger(1, 110, 228));
touchEvent("touchend", input, finger(1, 110, 228));
fire("focusin", input);
assert.deepEqual(taps.splice(0), [[100, 200]]);
// eframe focusing its input itself is no tap.
fire("focusin", input);
assert.deepEqual(taps, []);

// A drag that began on the input goes to egui, from where it began.
touchEvent("touchstart", input, finger(2, 110, 220));
assert.equal(touchEvent("touchmove", input, finger(2, 115, 224)), false, "within the slop it may still be a tap");
assert.equal(touchEvent("touchmove", input, finger(2, 150, 220)), true);
touchEvent("touchmove", input, finger(2, 160, 230));
assert.equal(touchEvent("touchend", input, finger(2, 160, 230)), true, "no focus or text menu after a drag");
fire("focusin", input);
assert.deepEqual(touches.splice(0), [
  ["start", 2, 100, 200],
  ["move", 2, 140, 200],
  ["move", 2, 150, 210],
  ["end", 2, 150, 210],
]);
assert.deepEqual(taps, [], "a drag is no tap");

// A second finger makes a drag a pinch: eframe's own listeners take over.
touchEvent("touchstart", input, finger(3, 110, 220));
touchEvent("touchmove", input, finger(3, 140, 220));
touchEvent("touchstart", canvas, finger(4, 300, 300), [finger(3, 140, 220), finger(4, 300, 300)]);
assert.deepEqual(touches.splice(0), [
  ["start", 3, 100, 200],
  ["move", 3, 130, 200],
  ["cancel", 3, 130, 200],
]);
touchEvent("touchend", canvas, finger(4, 300, 300));

// Held still: a long press (egui's context menu), not a tap.
touchEvent("touchstart", input, finger(5, 110, 220));
await sleep(HOLD + 40);
assert.equal(touchEvent("touchend", input, finger(5, 110, 220)), true);
fire("focusin", input);
assert.deepEqual(touches.splice(0), [
  ["long", 5, 100, 200],
  ["gone", 5, 100, 200],
]);
assert.deepEqual(taps, []);

// Android's own long press (`contextmenu`) makes it one at once, without the text field's popup.
touchEvent("touchstart", input, finger(6, 110, 220));
assert.equal(fire("contextmenu", input), true);
touchEvent("touchend", input, finger(6, 110, 220));
assert.deepEqual(touches.splice(0), [
  ["long", 6, 100, 200],
  ["gone", 6, 100, 200],
]);

// Touches on the canvas are eframe's.
touchEvent("touchstart", canvas, finger(7, 110, 220));
touchEvent("touchmove", canvas, finger(7, 200, 220));
assert.equal(touchEvent("touchend", canvas, finger(7, 200, 220)), false);
fire("focusin", input);
assert.deepEqual([taps, touches], [[], []]);

// A mouse or trackpad (an iPad's) focuses the input on the press.
fire("pointerdown", input, { pointerType: "mouse", button: 0, clientX: 30, clientY: 40 });
fire("focusin", input);
assert.deepEqual(taps.splice(0), [[20, 20]]);
// Its right click is no tap, and opens no text field menu.
fire("pointerdown", input, { pointerType: "mouse", button: 2, clientX: 30, clientY: 40 });
assert.equal(fire("contextmenu", input), true);
fire("focusin", input);
assert.deepEqual([taps, touches], [[], []]);

// The input lies over the fields, clipped to them; with none it shrinks back.
host.touchCover(new Float32Array([10, 20, 100, 18, 40, 60, 50, 20]));
assert.deepEqual(
  [input.style.left, input.style.top, input.style.width, input.style.height, input.style["clip-path"]],
  ["10px", "20px", "100px", "60px", "path('M0 0h100v18h-100zM30 40h50v20h-50z')"],
);
host.touchCover(new Float32Array([]));
assert.deepEqual([input.style.width, input.style.height, input.style["clip-path"]], ["1px", "1px", undefined]);

// iOS's pan to keep the focused input in view is undone once nothing types.
globalThis.scrollY = 120;
document.activeElement = input;
fire("scroll", globalThis);
assert.equal(scrolled, 0, "not while typing");
document.activeElement = canvas;
fire("scroll", globalThis);
assert.equal(scrolled, 1);

// Not on a desktop (a fine pointer).
globalThis.matchMedia = () => ({ matches: false });
assert.equal(host.touchInstall(canvas, HOLD, () => {}, () => {}), false);

console.log("touch: ok");
