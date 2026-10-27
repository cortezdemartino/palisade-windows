"use strict";
const test = require("node:test"),
  assert = require("node:assert/strict"),
  fs = require("node:fs"),
  vm = require("node:vm");
const { EventEmitter } = require("node:events");
const contract = require("../src/assist/native-protocol.cjs"),
  fixtures = require("./fixtures/assist-native-v1.json");
const turn = () => new Promise((resolve) => setImmediate(resolve));
const DOC = "12345678-1234-1234-1234-123456789abc",
  OTHER = "87654321-1234-1234-1234-123456789abc";
const status = {
  endpoint: { enrolled: false, endpointId: null, pin: "none" },
  capture: { operation: null, state: "idle", trackSid: null },
  controller: { session: null, state: "idle" },
};
test('destroyed contents revoke scopes without reading a destroyed main frame', async () => {
  const t = setup();
  await t.connect(DOC);
  await t.request('hello', { versions: [1] });
  t.contents.isDestroyed = () => true;
  Object.defineProperty(t.contents, 'mainFrame', { get() { throw Error('Destroyed frame'); } });
  assert.doesNotThrow(() => t.contents.emit('destroyed'));
  assert.equal(t.ipc.handlers.size, 0);
  assert(t.stops() > 0);
});
function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function setup(options = {}) {
  const ipc = Object.assign(new EventEmitter(), {
    handlers: new Map(),
    handle(channel, handler) {
      this.handlers.set(channel, handler);
    },
    removeHandler(channel) {
      this.handlers.delete(channel);
    },
  });
  const contents = Object.assign(new EventEmitter(), {
    mainFrame: { url: "https://palisade.onyxops.org/" },
    isDestroyed: () => false,
    getURL: () => contents.mainFrame.url,
    sent: [],
  });
  const event = { sender: contents, senderFrame: contents.mainFrame };
  let liveDocument = DOC,
    stops = 0,
    dispatch = async (request) =>
      request.op === "hello"
        ? {
            version: 1,
            features: {
              capture: false,
              clipboard: false,
              files: false,
              input: false,
            },
          }
        : status;
  const context = {
    module: { exports: {} },
    AbortController,
    setTimeout,
    clearTimeout,
    require: (name) =>
      name === "electron"
        ? { ipcMain: ipc }
        : name === "../security-policy.cjs"
          ? require("../src/security-policy.cjs")
          : name === "./native-protocol.cjs"
            ? contract
            : require(name),
  };
  vm.runInNewContext(
    fs.readFileSync(require.resolve("../src/assist/native-bridge.cjs"), "utf8"),
    context,
  );
  const { CHANNELS, installNativeBridge } = context.module.exports;
  contents.send = (channel, value) => {
    contents.sent.push([channel, value]);
    if (channel === CHANNELS.probe)
      queueMicrotask(() =>
        ipc.emit(CHANNELS.witness, event, {
          document: liveDocument,
          challenge: value.challenge,
        }),
      );
  };
  const bridge = installNativeBridge(contents, {
    enabled: true,
    dispatch: (...args) => dispatch(...args),
    stopAll: () => {
      stops++;
    },
    ...options,
  });
  const connect = (document) =>
    ipc.handlers.get(CHANNELS.connect)(event, { document });
  const request = async (
    op,
    fields = {},
    id = "request_" + Math.random().toString(36).slice(2),
  ) =>
    JSON.parse(
      await ipc.handlers.get(CHANNELS.request)(event, {
        document: liveDocument,
        text: JSON.stringify({
          p: contract.NATIVE_PROTOCOL,
          v: 1,
          id,
          op,
          ...fields,
        }),
      }),
    );
  return {
    ipc,
    contents,
    event,
    bridge,
    CHANNELS,
    connect,
    request,
    stops: () => stops,
    dispatch: (value) => {
      dispatch = value;
    },
    document: (value) => {
      liveDocument = value;
    },
  };
}
test("native API remains absent by default and accepts only the witnessed hosted main document", async () => {
  const disabled = setup({ enabled: false });
  assert.equal(disabled.ipc.handlers.size, 0);
  disabled.bridge.dispose();
  const t = setup();
  assert.equal(await t.connect(OTHER), false);
  assert.equal(await t.connect(DOC), true);
  assert.equal((await t.request("status")).error, "unsupported-version");
  assert((await t.request("hello", { versions: [1] })).ok);
  assert.equal(
    (await t.request("enroll.key", { origin: "https://another.example" }))
      .error,
    "issuer-mismatch",
  );
  await assert.rejects(async () =>
    t.ipc.handlers.get(t.CHANNELS.request)(
      { ...t.event, senderFrame: {} },
      { document: DOC, text: "{}" },
    ),
  );
  t.bridge.dispose();
  assert.equal(t.ipc.handlers.size, 0);
});
test("schema and response checks refuse generalized calls, duplicate correlation and backend secrets", async () => {
  const t = setup();
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  assert.equal(
    (await t.request("invoke", { command: "anything" })).error,
    "invalid-message",
  );
  assert((await t.request("status", {}, "request_reused_01")).ok);
  assert.equal(
    (await t.request("status", {}, "request_reused_01")).error,
    "stale-operation",
  );
  t.dispatch(async () => ({ ...status, privateKey: "must-never-be-returned" }));
  const result = await t.request("status");
  assert.equal(result.ok, false);
  assert(!JSON.stringify(result).includes("must-never"));
  assert(t.stops() > 0);
  t.bridge.dispose();
});
test("stop bypasses eight pending confirmations and aborts them before backend completion", async () => {
  const t = setup();
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  const release = deferred(),
    scopes = [];
  t.dispatch(async (_request, scope) => {
    scopes.push(scope);
    await release.promise;
    return { signature: fixtures.statements[0].signature };
  });
  const pending = Array.from({ length: 8 }, (_, index) =>
    t.request(
      "enroll.prove",
      {
        endpointId: "endpoint_fixture_01",
        challenge: fixtures.requests.valid[2].challenge,
        account: "@streamer:palisade.test",
        device: "DEVICE",
      },
      "request_pending_" + index,
    ),
  );
  await turn();
  assert.equal((await t.request("status")).error, "busy");
  assert((await t.request("stop")).ok);
  assert(scopes.every((scope) => scope.signal.aborted));
  for (const result of await Promise.all(pending))
    assert.equal(result.error, "stopped");
  release.resolve();
  await turn();
  t.bridge.dispose();
});
test("capture cancellation bypasses a full ordinary queue without ending unrelated requests", async () => {
  const t = setup();
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  const release = deferred();
  let captureScope;
  t.dispatch(async (request, scope) => {
    if (request.op === "capture.cancel") return {};
    if (request.op === "capture.start") captureScope = scope;
    await release.promise;
    return request.op === "capture.start"
      ? { operation: request.operation }
      : status;
  });
  const capture = t.request("capture.start", {
    operation: "capture_owned_01",
    room: "!room:palisade.test",
  });
  const reads = Array.from({ length: 7 }, (_, i) =>
    t.request("status", {}, "request_status_" + i),
  );
  await turn();
  assert(
    (await t.request("capture.cancel", { operation: "capture_owned_01" })).ok,
  );
  assert.equal((await capture).error, "cancelled");
  assert(captureScope.signal.aborted);
  release.resolve();
  for (const value of await Promise.all(reads)) assert(value.ok);
  t.bridge.dispose();
});
test("request timeout and reload reject late signatures and stale document expiry messages", async () => {
  const t = setup({ timeoutMs: 15 });
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  const late = deferred();
  let scope;
  t.dispatch(async (_request, context) => {
    scope = context;
    await late.promise;
    return { signature: fixtures.statements[0].signature };
  });
  const pending = t.request("enroll.prove", {
    ...fixtures.requests.valid[2],
    p: contract.NATIVE_PROTOCOL,
    v: 1,
    id: "request_timeout_01",
    op: "enroll.prove",
  });
  await new Promise((resolve) => setTimeout(resolve, 25));
  assert.equal((await pending).error, "timeout");
  assert(scope.signal.aborted);
  late.resolve();
  await turn();
  t.contents.emit(
    "did-start-navigation",
    {},
    "https://palisade.onyxops.org/",
    false,
    true,
  );
  t.document(OTHER);
  assert.equal(await t.connect(OTHER), true);
  t.dispatch(async (request) =>
    request.op === "hello"
      ? {
          version: 1,
          features: {
            capture: false,
            clipboard: false,
            files: false,
            input: false,
          },
        }
      : status,
  );
  await t.request("hello", { versions: [1] });
  t.ipc.emit(t.CHANNELS.expired, t.event, {
    document: DOC,
    id: "request_future_01",
  });
  assert((await t.request("status", {}, "request_future_01")).ok);
  t.bridge.dispose();
});
test("completed peer/capture scopes stay cancellable and only terminal events survive local cancellation", async () => {
  const t = setup();
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  let captureScope;
  t.dispatch(async (request, scope) => {
    if (request.op === "capture.start") {
      captureScope = scope;
      return { operation: request.operation };
    }
    return {};
  });
  await t.request("capture.start", {
    operation: "capture_owned_01",
    room: "!room:palisade.test",
  });
  assert(
    captureScope.emit({
      event: "capture.status",
      operation: "capture_owned_01",
      state: "live",
      reason: null,
    }),
  );
  await t.request("capture.cancel", { operation: "capture_owned_01" });
  assert(captureScope.signal.aborted);
  assert.equal(
    captureScope.emit({
      event: "capture.status",
      operation: "capture_owned_01",
      state: "live",
      reason: null,
    }),
    false,
  );
  assert(
    captureScope.emit({
      event: "capture.status",
      operation: "capture_owned_01",
      state: "cancelled",
      reason: "picker-cancelled",
    }),
  );
  t.contents.emit(
    "did-start-navigation",
    {},
    "https://palisade.onyxops.org/",
    false,
    true,
  );
  assert.equal(
    captureScope.emit({
      event: "capture.status",
      operation: "capture_owned_01",
      state: "stopped",
      reason: "reload",
    }),
    false,
  );
  t.bridge.dispose();
});
test("retired source events cannot stop a replacement or target a different source, and terminal events free capacity", async () => {
  const t = setup();
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  let scope;
  t.dispatch(async (request, context) => {
    if (request.op === "capture.start") {
      scope = context;
      return { operation: request.operation };
    }
    return {};
  });
  await t.request("capture.start", {
    operation: "capture_reused_01",
    room: "!room:palisade.test",
  });
  const old = scope;
  assert.equal(
    old.emit({
      event: "capture.status",
      operation: "capture_other_01",
      state: "live",
      reason: null,
    }),
    false,
  );
  await t.request("capture.cancel", { operation: "capture_reused_01" });
  await t.request("capture.start", {
    operation: "capture_reused_01",
    room: "!room:palisade.test",
  });
  assert.equal(
    old.emit({
      event: "capture.status",
      operation: "capture_reused_01",
      state: "stopped",
      reason: "local-stop",
    }),
    false,
  );
  assert(
    scope.emit({
      event: "capture.status",
      operation: "capture_reused_01",
      state: "stopped",
      reason: "source-lost",
    }),
  );
  assert.equal(
    scope.emit({
      event: "capture.status",
      operation: "capture_reused_01",
      state: "live",
      reason: null,
    }),
    false,
  );
  for (let i = 0; i < 20; i++) {
    const operation = "capture_terminal_" + i;
    assert(
      (
        await t.request("capture.start", {
          operation,
          room: "!room:palisade.test",
        })
      ).ok,
    );
    assert(
      scope.emit({
        event: "capture.status",
        operation,
        state: "stopped",
        reason: "source-lost",
      }),
    );
    assert.equal(
      scope.emit({
        event: "capture.status",
        operation,
        state: "stopped",
        reason: "source-lost",
      }),
      false,
    );
  }
  t.bridge.dispose();
});
test("global stop permits confirmed terminal cleanup events but never old live events or post-reload completions", async () => {
  const cleanup = deferred();
  let completion;
  const t = setup({
    stopAll: (_reason, scope) => {
      completion = scope;
      return cleanup.promise;
    },
  });
  await t.connect(DOC);
  await t.request("hello", { versions: [1] });
  assert((await t.request("stop")).ok);
  assert.equal(
    completion.emit({
      event: "lifecycle",
      session: null,
      state: "active",
      reason: null,
    }),
    false,
  );
  assert(
    completion.emit({
      event: "lifecycle",
      session: null,
      state: "stopped",
      reason: "local-stop",
    }),
  );
  t.contents.emit(
    "did-start-navigation",
    {},
    "https://palisade.onyxops.org/",
    false,
    true,
  );
  assert.equal(
    completion.emit({
      event: "lifecycle",
      session: null,
      state: "stopped",
      reason: "local-stop",
    }),
    false,
  );
  cleanup.resolve();
  await turn();
  t.bridge.dispose();
});
