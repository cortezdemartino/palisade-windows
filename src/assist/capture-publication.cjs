"use strict";
const { createIsolatedCapture } = require("./isolated-capture.cjs");
const { NativeBridgeError } = require("./native-protocol.cjs");
// Main-owned source lifecycle for docs/49. A page-supplied SID is tentative
// metadata, never evidence of source ownership or native permission.
function createCapturePublication({
  enabled = false,
  createCapture = createIsolatedCapture,
  onInvalidate = () => {},
  onFatal = () => {},
  ...options
}) {
  let active = null,
    disposed = false,
    failed = false;
  const retired = new Set();
  const settling = new Set();
  const fail = (code) => {
    throw new NativeBridgeError(code);
  };
  const capture = createCapture({
    ...options,
    enabled,
    onState: (state, operation) => {
      if (
        ["ended", "unavailable"].includes(state) &&
        active &&
        operation === active.id
      )
        finish(active, "stopped", "source-lost");
    },
  });
  function finish(operation, state, reason) {
    if (active !== operation || operation.ended) return false;
    operation.ended = true;
    active = null;
    operation.scope.signal.removeEventListener("abort", operation.abort);
    // Close main-owned admission immediately. A native fence acknowledgement
    // may be asynchronous; never report terminal success before it arrives.
    let fence;
    const failedFence = () => {
      failed = true;
      state = "failed";
      reason = "error";
      try { onFatal(); } catch {}
    };
    try {
      fence = onInvalidate(operation.snapshot, { operation: operation.id, room: operation.room });
    } catch {
      failedFence();
    }
    try { capture.stop(); } catch { failedFence(); }
    const publish = () => {
      operation.scope.emit({
        event: "capture.status",
        operation: operation.id,
        state,
        reason,
      });
    };
    if (fence && typeof fence.then === "function") {
      const completion = Promise.resolve(fence).then(() => { publish(); return true; }, () => {
        failedFence(); publish(); throw new NativeBridgeError("stopped");
      });
      settling.add(completion);
      void completion.finally(() => settling.delete(completion)).catch(() => {});
      return completion;
    }
    publish();
    return true;
  }
  async function dispatch(request, scope) {
    if (!enabled || disposed || failed) fail("capture-unavailable");
    if (request.op === "capture.start") {
      if (settling.size) await Promise.all([...settling]);
      if (failed || disposed) fail("capture-unavailable");
      if (retired.has(request.operation)) fail("stale-operation");
      if (active || capture.busy() || retired.size >= 128) fail("busy");
      if (scope.signal.aborted) fail("cancelled");
      const operation = {
        id: request.operation,
        room: request.room,
        trackSid: null,
        state: "picking",
        scope,
        ended: false,
        snapshot: null,
      };
      operation.abort = () =>
        finish(
          operation,
          operation.state === "picking" ? "cancelled" : "stopped",
          "local-stop",
        );
      retired.add(operation.id);
      active = operation;
      scope.signal.addEventListener("abort", operation.abort, { once: true });
      if (
        !scope.emit({
          event: "capture.status",
          operation: operation.id,
          state: "picking",
          reason: null,
        })
      ) {
        finish(operation, "cancelled", "local-stop");
        fail("cancelled");
      }
      try {
        await capture.request({ publicOperation: operation.id });
        if (active !== operation || operation.ended || scope.signal.aborted)
          fail("cancelled");
        const source = capture.selected();
        if (!source) fail("capture-unavailable");
        operation.state = "live";
        operation.snapshot = Object.freeze({
          operation: operation.id,
          room: operation.room,
          trackSid: null,
          source,
        });
        scope.emit({
          event: "capture.status",
          operation: operation.id,
          state: "live",
          reason: null,
        });
        return { operation: operation.id };
      } catch (error) {
        if (active === operation)
          await finish(operation, "cancelled", "picker-cancelled");
        throw error instanceof NativeBridgeError
          ? error
          : new NativeBridgeError("cancelled");
      }
    }
    if (
      !["capture.bind", "capture.cancel", "capture.stop"].includes(request.op)
    )
      fail("invalid-message");
    const operation = active;
    if (!operation || operation.id !== request.operation) {
      // A safety abort can have already stopped this exact operation at the
      // bridge, before its cancellation RPC arrives. Never affect a successor.
      if (request.op !== "capture.bind" && retired.has(request.operation))
        return {};
      fail("stale-operation");
    }
    if (request.op === "capture.bind") {
      if (
        operation.state !== "live" ||
        operation.trackSid !== null ||
        scope.signal.aborted
      )
        fail("stale-operation");
      operation.trackSid = request.trackSid;
      operation.snapshot = Object.freeze({
        ...operation.snapshot,
        trackSid: request.trackSid,
      });
      return {};
    }
    await finish(
      operation,
      request.op === "capture.cancel" ? "cancelled" : "stopped",
      request.op === "capture.cancel" ? "picker-cancelled" : "local-stop",
    );
    return {};
  }
  function stop(reason = "local-stop") {
    if (active) return finish(active, "stopped", reason);
    if (settling.size) return Promise.all([...settling]);
  }
  function dispose() {
    if (disposed) return;
    disposed = true;
    stop();
    capture.dispose();
  }
  return {
    dispatch,
    stop,
    dispose,
    current: () => active?.snapshot || null,
    status: () => ({
      operation: active?.id || null,
      state: active?.state || "idle",
      trackSid: active?.trackSid || null,
    }),
  };
}
module.exports = { createCapturePublication };
