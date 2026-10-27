"use strict";
const test = require("node:test"),
  assert = require("node:assert/strict"),
  fs = require("node:fs"),
  vm = require("node:vm");
const { NativeBridgeError } = require("../src/assist/native-protocol.cjs");
const turn = () => new Promise((resolve) => setImmediate(resolve));
function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function setup(options = {}) {
  let onState,
    busy = false,
    starts = [],
    stopped = 0,
    invalidated = [],
    order = [];
  const source = Object.freeze({
    operation: "private_nonce",
    id: "screen:10:0",
    kind: "screen",
    displayId: "10",
  });
  let request = async (value) => {
    starts.push(value);
    busy = true;
  };
  const context = {
    module: { exports: {} },
    Object,
    require: (name) =>
      name === "./native-protocol.cjs" ? { NativeBridgeError } : {},
  };
  vm.runInNewContext(
    fs.readFileSync(
      require.resolve("../src/assist/capture-publication.cjs"),
      "utf8",
    ),
    context,
  );
  const publication = context.module.exports.createCapturePublication({
    enabled: true,
    createCapture: (options) => {
      onState = options.onState;
      return {
        request: (...args) => request(...args),
        selected: () => source,
        busy: () => busy,
        stop: () => {
          stopped++;
          busy = false;
          order.push("source-stop");
          onState("ended", starts.at(-1)?.publicOperation);
        },
        dispose() {},
      };
    },
    onInvalidate: (value) => {
      invalidated.push(value);
      order.push("revoke");
    },
    ...options,
  });
  const makeScope = () => {
    const abort = new AbortController(),
      events = [];
    return {
      abort,
      events,
      signal: abort.signal,
      emit: (value) => {
        events.push(value);
        return true;
      },
    };
  };
  const start = async (id = "capture_fixture_01", scope = makeScope()) => {
    const result = await publication.dispatch(
      { op: "capture.start", operation: id, room: "!fixture:palisade.test" },
      scope,
    );
    return { result, scope };
  };
  return {
    publication,
    makeScope,
    start,
    source,
    starts,
    order,
    invalidated,
    stopped: () => stopped,
    emit: (state, operation = publication.current()?.operation) =>
      onState(state, operation),
    request: (value) => {
      request = value;
    },
  };
}
test("publication binds one tentative SID to the native source and revokes children before source teardown", async () => {
  const t = setup();
  const { scope } = await t.start();
  assert.equal(t.starts[0].publicOperation, "capture_fixture_01");
  assert.equal(t.publication.current().source, t.source);
  assert.equal(t.publication.current().trackSid, null);
  await t.publication.dispatch(
    {
      op: "capture.bind",
      operation: "capture_fixture_01",
      trackSid: "TR_fixture01",
    },
    t.makeScope(),
  );
  assert.equal(t.publication.current().trackSid, "TR_fixture01");
  for (const operation of ["capture_fixture_01", "capture_other_01"])
    await assert.rejects(
      t.publication.dispatch(
        { op: "capture.bind", operation, trackSid: "TR_other01" },
        t.makeScope(),
      ),
      (error) => error.code === "stale-operation",
    );
  t.emit("ended");
  assert.equal(t.publication.current(), null);
  assert.deepEqual(t.order, ["revoke", "source-stop"]);
  assert.equal(t.invalidated[0].trackSid, "TR_fixture01");
  assert.equal(scope.events.at(-1).reason, "source-lost");
  await assert.rejects(t.start(), (error) => error.code === "stale-operation");
  t.publication.dispose();
});
test("abort cancels selection immediately and its late completion cannot affect a successor", async () => {
  const t = setup(),
    old = deferred(),
    scope = t.makeScope();
  t.request(() => old.promise);
  const pending = t.start("capture_cancel_01", scope);
  await turn();
  scope.abort.abort();
  // The real picker resolves on abort; simulate a delayed uncancellable source enumeration.
  t.request(async () => {});
  await t.start("capture_successor_01");
  old.resolve();
  await assert.rejects(pending, (error) => error.code === "cancelled");
  assert.equal(t.publication.current().operation, "capture_successor_01");
  await t.publication.dispatch(
    { op: "capture.stop", operation: "capture_cancel_01" },
    t.makeScope(),
  );
  assert.equal(t.publication.current().operation, "capture_successor_01");
  assert.equal(t.stopped(), 1);
  t.publication.dispose();
});
test("disabled, busy, pre-aborted and unsupported calls do not select a source", async () => {
  const disabled = setup({ enabled: false });
  await assert.rejects(
    disabled.start(),
    (error) => error.code === "capture-unavailable",
  );
  assert.equal(disabled.starts.length, 0);
  disabled.publication.dispose();
  const t = setup(),
    scope = t.makeScope();
  scope.abort.abort();
  await assert.rejects(
    t.start("capture_aborted_01", scope),
    (error) => error.code === "cancelled",
  );
  await assert.rejects(
    t.publication.dispatch({ op: "filesystem.read" }, scope),
    (error) => error.code === "invalid-message",
  );
  await t.start();
  await assert.rejects(
    t.start("capture_another_01"),
    (error) => error.code === "busy",
  );
  assert.equal(t.starts.length, 1);
  t.publication.dispose();
});
test("revocation failure latches capture unavailable and invokes global fail-closed cleanup", async () => {
  let fatal = 0;
  const t = setup({
    onInvalidate: () => {
      throw Error("Fixture failure");
    },
    onFatal: () => {
      fatal++;
    },
  });
  const { scope } = await t.start();
  t.emit("ended");
  assert.equal(fatal, 1);
  assert.equal(t.stopped(), 1);
  assert.equal(scope.events.at(-1).state, "failed");
  await assert.rejects(
    t.start("capture_after_failure"),
    (error) => error.code === "capture-unavailable",
  );
  t.publication.dispose();
});
test("old source-loss callbacks cannot end a later publication", async () => {
  const t = setup();
  await t.start("capture_original_01");
  await t.publication.dispatch(
    { op: "capture.stop", operation: "capture_original_01" },
    t.makeScope(),
  );
  await t.start("capture_successor_01");
  t.emit("ended", "capture_original_01");
  assert.equal(t.publication.current().operation, "capture_successor_01");
  assert.equal(t.stopped(), 1);
  t.publication.dispose();
});
