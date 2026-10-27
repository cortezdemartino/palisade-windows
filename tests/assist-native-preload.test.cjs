"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const { EventEmitter } = require("node:events");
const { randomUUID } = require("node:crypto");
const source = fs.readFileSync(
  require.resolve("../src/chat-notifications-preload.cjs"),
  "utf8",
);
const prefix = "palisade:assist-native-";
const wire = (op, id = "request_fixture_01") =>
  JSON.stringify({ p: "palisade.assist.native", v: 1, id, op });
const turn = () => new Promise((resolve) => setImmediate(resolve));
function setup(overrides = {}) {
  const api = {},
    events = new EventEmitter(),
    ipc = new EventEmitter(),
    invocations = [],
    messages = [],
    timers = new Map();
  let invoke = async (channel) =>
    channel === prefix + "connect" ? true : "{}";
  ipc.invoke = (...args) => {
    invocations.push(args);
    return invoke(...args);
  };
  ipc.send = (...args) => messages.push(args);
  let timerId = 0;
  const context = {
    Object,
    crypto: { randomUUID },
    process: { isMainFrame: true, argv: ["--palisade-assist-native"] },
    location: { origin: "https://palisade.onyxops.org" },
    window: { addEventListener: (name, fn) => events.on(name, fn) },
    setTimeout: (fn, delay) => {
      timers.set(++timerId, { fn, delay });
      return timerId;
    },
    clearTimeout: (id) => timers.delete(id),
    require: () => ({
      ipcRenderer: ipc,
      contextBridge: {
        exposeInMainWorld: (name, value) => {
          api[name] = value;
        },
      },
    }),
    ...overrides,
  };
  vm.runInNewContext(source, context);
  return {
    api,
    events,
    ipc,
    invocations,
    messages,
    timers,
    dispatch: (value) => {
      invoke = value;
    },
  };
}
test("native preload exposes exactly two functions only in enabled trusted main documents", async () => {
  const t = setup();
  assert.equal(
    Object.keys(t.api.palisadeAssistNative).sort().join(","),
    "request,subscribe",
  );
  assert.equal(
    Object.keys(t.api.palisadeDesktopAssist).sort().join(","),
    "getCapabilities,onCapabilitiesChanged,protocol",
  );
  const connection = t.invocations[0][1];
  t.ipc.emit(
    prefix + "probe",
    { privateElectronEvent: true },
    { challenge: "a".repeat(48) },
  );
  assert.deepEqual(JSON.parse(JSON.stringify(t.messages[0])), [
    prefix + "witness",
    { document: connection.document, challenge: "a".repeat(48) },
  ]);
  assert.equal(await t.api.palisadeAssistNative.request(wire("status")), "{}");
  assert.equal(t.invocations[1][1].document, connection.document);
  for (const change of [
    { process: { isMainFrame: true, argv: [] } },
    { process: { isMainFrame: false, argv: ["--palisade-assist-native"] } },
    { location: { origin: "https://other.example" } },
  ]) {
    assert.equal(setup(change).api.palisadeAssistNative, undefined);
  }
});
test("preload bounds requests and listeners, admits stop while full, and expires the exact private document call", async () => {
  const t = setup(),
    native = t.api.palisadeAssistNative;
  t.dispatch(() => new Promise(() => {}));
  const pending = Array.from({ length: 8 }, (_, i) =>
    native.request(wire("status", "request_pending_" + i)),
  );
  await turn();
  await assert.rejects(native.request(wire("status")), /busy/);
  const stop = native.request(wire("stop", "request_stop_01"));
  await turn();
  assert.equal(
    t.invocations.filter((args) => args[0] === prefix + "request").length,
    9,
  );
  await assert.rejects(native.request(wire("stop", "request_stop_02")), /busy/);
  const first = t.timers.values().next().value;
  assert.equal(first.delay, 120000);
  first.fn();
  await assert.rejects(pending[0], /timeout/);
  assert.equal(t.messages[0][0], prefix + "expired");
  assert.equal(t.messages[0][1].id, "request_pending_0");
  assert.equal(t.messages[0][1].document, t.invocations[0][1].document);
  for (const text of [
    null,
    "",
    "{",
    "a".repeat(32769),
    wire("status", "short"),
  ])
    await assert.rejects(native.request(text));
  const results = Promise.allSettled([...pending.slice(1), stop]);
  t.events.emit("unload");
  assert((await results).every((result) => result.status === "rejected"));
  assert.equal(t.timers.size, 0);
  assert.equal(t.ipc.listenerCount(prefix + "probe"), 0);
  assert.equal(t.ipc.listenerCount(prefix + "event"), 0);
});
test("preload strips Electron events, drops replayed events and disconnects subscribers on unload", async () => {
  const t = setup(),
    native = t.api.palisadeAssistNative,
    seen = [];
  const envelope = (text) => ({ document: t.invocations[0][1].document, text });
  const off = native.subscribe((...args) => seen.push(args));
  const event = JSON.stringify({
    p: "palisade.assist.native",
    v: 1,
    seq: 1,
    event: "lifecycle",
    session: null,
    state: "stopped",
    reason: "local-stop",
  });
  t.ipc.emit(prefix + "event", {}, { document: randomUUID(), text: event });
  t.ipc.emit(prefix + "event", {}, event);
  assert.equal(seen.length, 0);
  t.ipc.emit(prefix + "event", { sender: "private" }, envelope(event));
  t.ipc.emit(prefix + "event", {}, envelope(event));
  t.ipc.emit(prefix + "event", {}, envelope("{"));
  assert.equal(seen.length, 1);
  assert.equal(seen[0].length, 1);
  assert.equal(seen[0][0], event);
  off();
  t.ipc.emit(
    prefix + "event",
    {},
    envelope(event.replace('"seq":1', '"seq":2')),
  );
  assert.equal(seen.length, 1);
  for (let i = 0; i < 9; i++) native.subscribe(() => seen.push(i));
  t.ipc.emit(
    prefix + "event",
    {},
    envelope(event.replace('"seq":1', '"seq":3')),
  );
  assert.equal(seen.length, 9);
  t.events.emit("unload");
  await assert.rejects(native.request(wire("status")));
  assert.equal(t.ipc.listenerCount(prefix + "event"), 0);
});
