"use strict";
const { ipcMain } = require("electron");
const { randomBytes } = require("node:crypto");
const { performance } = require("node:perf_hooks");
const { isPalisadeUrl, PALISADE_ORIGIN } = require("../security-policy.cjs");
const protocol = require("./native-protocol.cjs");
const extension = require("./native-extension.cjs");
const CHANNELS = Object.freeze({
  connect: "palisade:assist-native-connect",
  request: "palisade:assist-native-request",
  expired: "palisade:assist-native-expired",
  probe: "palisade:assist-native-probe",
  witness: "palisade:assist-native-witness",
  event: "palisade:assist-native-event",
});
const UUID = /^[a-f0-9-]{36}$/;
const ID = /^[A-Za-z0-9_-]{8,64}$/;
const SAFETY = new Set(["capture.cancel", "capture.stop", "peer.close"]);
const isExtension = op => typeof op === 'string' && (op === 'ext.hello' || op.startsWith('resource.'));
const isSafety = request => SAFETY.has(request.op) || (isExtension(request.op) && extension.isExtSafety(request.op, request));
const keyOf = request => (isExtension(request.op) ? 'ext:' : 'native:') + request.id;
function cancels(request, other) {
  if (request.operation) return other.operation === request.operation;
  if (request.session !== other.session) return false;
  if (request.op === 'resource.cancel') return isExtension(other.op) && other.transfer === request.transfer;
  if (request.op === 'resource.sync') return other.op === 'resource.sync' && other.grantId === request.grantId;
  if (request.op === 'peer.close') {
    if (isExtension(other.op)) return false;
    if (other.op === 'grant.offer') {
      try { if (!['control_mouse', 'control_keyboard'].includes(JSON.parse(other.grant).grant.capability)) return false; }
      catch {} // Malformed offers have no resource authority to preserve.
    }
    return true;
  }
  return false;
}
const exact = (value, fields) =>
  value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.keys(value).sort().join(",") === [...fields].sort().join(",");
// This gate transports canonical messages. All authority, signatures, consent,
// source/peer ownership and OS work belong to the native coordinator.
function installNativeBridge(
  contents,
  {
    enabled = false,
    dispatch,
    stopAll,
    origin = PALISADE_ORIGIN,
    now = () => performance.now(),
    timeoutMs = protocol.NATIVE_LIMITS.requestTimeoutMs,
  } = {},
) {
  if (enabled !== true) return { dispose() {} };
  if (
    typeof dispatch !== "function" ||
    typeof stopAll !== "function" ||
    origin !== PALISADE_ORIGIN
  )
    throw Error("Native coordinator and fixed origin required");
  let disposed = false,
    generation = 0,
    document = null,
    negotiated = false,
    extensionVersion = null,
    sequence = 0,
    extensionSequence = 0,
    rateTime = now(),
    tokens = 64,
    shutdown = null,
    failed = false;
  const pending = new Map(),
    probes = new Map(),
    seen = new Map(),
    lifetimes = new Map(),
    owners = new Map();
  const lifetimeKey = (request) =>
    request.op === "capture.start"
      ? "capture:" + request.operation
      : request.op === "peer.offer"
        ? "peer:" + request.session
        : null;
  const trusted = (event) =>
    !disposed &&
    !contents.isDestroyed() &&
    event.sender === contents &&
    event.senderFrame === contents.mainFrame &&
    isPalisadeUrl(contents.getURL()) &&
    isPalisadeUrl(event.senderFrame?.url);
  const reply = (id, op, result, error, version = extensionVersion || 1, offered = null) => {
    const ext = isExtension(op);
    const envelope = { p: ext ? extension.EXT_PROTOCOL : protocol.NATIVE_PROTOCOL, v: ext && op !== 'ext.hello' ? version : 1, id };
    const text = JSON.stringify(
      error
        ? { ...envelope, ok: false, error }
        : { ...envelope, ok: true, result },
    );
    if (ext) {
      extension.parseExtResponse(text, op, op === 'ext.hello' ? 3 : version);
      if (!error && op === 'ext.hello' && offered && !offered.includes(result.version)) throw Error('Unoffered extension version');
    } else protocol.parseResponse(text, op);
    return text;
  };
  const failure = (id, op, error, version) => reply(id, op, null, error, version);
  function settle(entry, error, result) {
    if (entry.done) return;
    entry.done = true;
    clearTimeout(entry.timer);
    pending.delete(keyOf(entry.request));
    if (error) {
      entry.abort.abort(error);
      if (lifetimes.get(lifetimeKey(entry.request)) === entry)
        lifetimes.delete(lifetimeKey(entry.request));
    }
    try {
      entry.resolve(reply(entry.request.id, entry.request.op, result, error, entry.request.v, entry.request.versions));
    } catch {
      entry.resolve(failure(entry.request.id, entry.request.op, "internal"));
    }
  }
  function invalidate(reason) {
    generation++;
    negotiated = false;
    extensionVersion = null;
    extensionSequence = 0;
    document = null;
    seen.clear();
    for (const probe of probes.values()) {
      clearTimeout(probe.timer);
      probe.resolve(false);
    }
    probes.clear();
    for (const entry of [...pending.values()]) settle(entry, reason);
    for (const entry of lifetimes.values()) entry.abort.abort(reason);
    lifetimes.clear();
    owners.clear();
  }
  function end(reason, keepDocument = false) {
    const previousDocument = keepDocument ? document : null;
    invalidate("stopped");
    document = previousDocument;
    if (!shutdown) {
      const stopped = {
        generation,
        document,
        frame: contents.isDestroyed() ? null : contents.mainFrame,
        abort: new AbortController(),
        done: false,
        request: { op: "stop" },
      };
      const completion = Object.freeze({
        emit: (payload) =>
          ["stopped", "cancelled", "failed"].includes(payload?.state) &&
          emit(stopped, payload),
      });
      try {
        shutdown = Promise.resolve(stopAll(reason, completion))
          .catch(() => {
            failed = true;
          })
          .finally(() => {
            stopped.done = true;
            shutdown = null;
          });
      } catch {
        failed = true;
      }
    }
  }
  function current(entry) {
    return (
      !disposed &&
      !entry.done &&
      !entry.abort.signal.aborted &&
      entry.generation === generation &&
      entry.document === document &&
      !contents.isDestroyed() &&
      entry.frame === contents.mainFrame &&
      isPalisadeUrl(contents.getURL())
    );
  }
  function emit(entry, payload) {
    const key = lifetimeKey(entry.request);
    if (entry.done && !key) return false;
    if (key && owners.get(key) !== entry) return false;
    const capture =
      entry.request.op === "capture.start" &&
      payload?.event === "capture.status" &&
      payload.operation === entry.request.operation;
    const peer =
      entry.request.op === "peer.offer" &&
      ["peer.candidate", "lifecycle"].includes(payload?.event) &&
      payload.session === entry.request.session;
    const consent =
      entry.request.op === "grant.offer" &&
      payload?.event === "lifecycle" &&
      payload.session === entry.request.session;
    const resource = isExtension(entry.request.op) && entry.request.session && payload?.session === entry.request.session &&
      ['resource.status', 'sync.status'].includes(payload?.event);
    if (entry.request.op !== "stop" && !capture && !peer && !consent && !resource)
      return false;
    const terminal = (entry.request.op === 'peer.offer' && payload?.event === 'lifecycle' && payload.state === 'suspended') || ["stopped", "cancelled", "failed"].includes(
      payload?.state,
    );
    if (
      !entry.document ||
      entry.generation !== generation ||
      entry.document !== document ||
      disposed ||
      contents.isDestroyed() ||
      entry.frame !== contents.mainFrame ||
      !isPalisadeUrl(contents.getURL()) ||
      !isPalisadeUrl(entry.frame?.url) ||
      (entry.abort.signal.aborted && !terminal)
    )
      return false;
    try {
      const ext = isExtension(entry.request.op);
      const message = JSON.stringify({
        ...payload,
        p: ext ? extension.EXT_PROTOCOL : protocol.NATIVE_PROTOCOL,
        v: ext ? entry.request.v : 1,
        seq: ext ? ++extensionSequence : ++sequence,
      });
      if (ext) extension.parseExtEvent(message, entry.request.v);
      else protocol.parseEvent(message);
      contents.send(CHANNELS.event, {
        document: entry.document,
        text: message,
      });
      if (key && terminal) {
        if (lifetimes.get(key) === entry) lifetimes.delete(key);
        if (owners.get(key) === entry) owners.delete(key);
        if (!entry.done) settle(entry, "stopped");
      }
      return true;
    } catch {
      end("error");
      return false;
    }
  }
  // The live isolated preload answers a fresh challenge with its own document
  // nonce. A connect message queued by a destroyed document cannot become current.
  ipcMain.handle(CHANNELS.connect, (event, message) => {
    if (
      !trusted(event) ||
      !exact(message, ["document"]) ||
      !UUID.test(message.document) ||
      probes.size >= 2
    )
      return false;
    const challenge = randomBytes(24).toString("hex"),
      epoch = generation;
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        probes.delete(challenge);
        resolve(false);
      }, 2500);
      timer.unref?.();
      probes.set(challenge, {
        epoch,
        document: message.document,
        frame: event.senderFrame,
        resolve,
        timer,
      });
      contents.send(CHANNELS.probe, { challenge });
    });
  });
  const witness = (event, message) => {
    if (!trusted(event) || !exact(message, ["document", "challenge"])) return;
    const probe = probes.get(message.challenge);
    if (!probe) return;
    probes.delete(message.challenge);
    clearTimeout(probe.timer);
    if (
      probe.epoch !== generation ||
      probe.frame !== event.senderFrame ||
      probe.document !== message.document
    ) {
      probe.resolve(false);
      return;
    }
    if (document && document !== probe.document) {
      end("reload");
      probe.resolve(false);
      return;
    }
    document = probe.document;
    probe.resolve(true);
  };
  ipcMain.on(CHANNELS.witness, witness);
  const expired = (event, message) => {
    if (
      !trusted(event) ||
      (!exact(message, ["document", "id"]) && !exact(message, ["document", "id", "protocol"])) ||
      document !== message.document
    )
      return;
    if (message.protocol !== undefined && ![protocol.NATIVE_PROTOCOL, extension.EXT_PROTOCOL].includes(message.protocol)) return;
    const entry = pending.get((message.protocol === extension.EXT_PROTOCOL ? 'ext:' : 'native:') + message.id);
    if (entry && entry.document === message.document) settle(entry, "timeout");
  };
  ipcMain.on(CHANNELS.expired, expired);
  ipcMain.handle(CHANNELS.request, (event, message) => {
    if (
      !trusted(event) ||
      !exact(message, ["document", "text"]) ||
      message.document !== document ||
      typeof message.text !== "string"
    )
      throw Error("Native bridge unavailable");
    if (
      !message.text.length ||
      message.text.length > protocol.NATIVE_LIMITS.messageChars
    )
      throw Error("Invalid native message");
    let request;
    const ext = extension.protocolOf(message.text) === 'ext';
    try {
      request = ext ? extension.parseExtRequest(message.text, extensionVersion || 1) : protocol.parseRequest(message.text);
    } catch (error) {
      let id, malformed;
      try {
        malformed = JSON.parse(message.text); id = malformed.id;
      } catch {}
      if (!ID.test(id || "")) throw Error("Invalid native message");
      return failure(
        id,
        ext && isExtension(malformed?.op) ? malformed.op : ext ? 'ext.hello' : "status",
        (ext ? extension.EXT_ERRORS : protocol.NATIVE_ERRORS).includes(error.code)
          ? error.code
          : "invalid-message", extensionVersion || 1,
      );
    }
    const id = request.id;
    const requestKey = keyOf(request);
    if (pending.has(requestKey) || seen.has(requestKey))
      return failure(id, request.op, "stale-operation");
    if (request.op === "enroll.key" && request.origin !== origin)
      return failure(id, request.op, "issuer-mismatch");
    if (request.op === "stop") {
      end("local-stop", true);
      return reply(id, request.op, {});
    }
    if (isSafety(request)) {
      for (const [key, entry] of lifetimes) {
        if (
          cancels(request, entry.request)
        ) {
          entry.abort.abort("cancelled");
          lifetimes.delete(key);
        }
      }
      for (const entry of [...pending.values()]) {
        if (
          cancels(request, entry.request)
        )
          settle(entry, "cancelled");
      }
      if (
        [...pending.values()].filter((entry) => isSafety(entry.request))
          .length >= 4
      )
        return failure(id, request.op, "busy");
    } else {
      if (failed || shutdown)
        return failure(id, request.op, failed ? "internal" : "busy");
      tokens = Math.min(64, tokens + Math.max(0, now() - rateTime) * 0.032);
      rateTime = now();
      if (
        tokens < 1 ||
        [...pending.values()].filter((entry) => !isSafety(entry.request))
          .length >= protocol.NATIVE_LIMITS.outstanding
      )
        return failure(id, request.op, "busy");
      tokens--;
    }
    if (request.op === "hello" && !request.versions.includes(1))
      return failure(id, request.op, "unsupported-version");
    if (request.op !== "hello" && !isSafety(request) && !negotiated)
      return failure(id, request.op, "unsupported-version");
    if (ext && request.op !== 'ext.hello' && (!extensionVersion || request.v !== extensionVersion))
      return failure(id, request.op, 'unsupported-version', request.v);
    const key = lifetimeKey(request);
    if (key && lifetimes.has(key))
      return failure(id, request.op, "stale-operation");
    if (key && lifetimes.size >= 16) return failure(id, request.op, "busy");
    // Bounded replay memory; IDs are correlation only, never authority. Native
    // grants/nonces/generations enforce security across the entire session.
    seen.set(requestKey, now());
    while (seen.size > 4096) seen.delete(seen.keys().next().value);
    return new Promise((resolve) => {
      const entry = {
        request,
        resolve,
        generation,
        document,
        frame: event.senderFrame,
        abort: new AbortController(),
        done: false,
      };
      entry.timer = setTimeout(() => settle(entry, "timeout"), timeoutMs);
      entry.timer.unref?.();
      pending.set(requestKey, entry);
      if (key) {
        lifetimes.set(key, entry);
        owners.set(key, entry);
        // Retired cancellation tails are correlation only. Eviction rejects
        // their late events and can never grant native authority.
        while (owners.size > 4096) {
          const retired = [...owners].find(
            ([ownerKey]) => !lifetimes.has(ownerKey),
          );
          if (!retired) break;
          owners.delete(retired[0]);
        }
      }
      const scope = Object.freeze({
        signal: entry.abort.signal,
        generation: entry.generation,
        emit: (payload) => emit(entry, payload),
        retire: () => {
          // Trusted runtime only: retire an intentionally replaced connection
          // without publishing an unscoped terminal event about its successor.
          if (!key || owners.get(key) !== entry) return false;
          if (lifetimes.get(key) === entry) lifetimes.delete(key);
          owners.delete(key);
          entry.abort.abort('replaced');
          if (!entry.done) settle(entry, 'cancelled');
          return true;
        },
      });
      Promise.resolve()
        .then(() => {
          if (!current(entry)) return;
          return dispatch(request, scope);
        })
        .then(
          (result) => {
            if (!current(entry)) return;
            try {
              reply(id, request.op, result, null, request.v, request.versions);
            } catch {
              end("error");
              return;
            }
            if (request.op === "hello") negotiated = true;
            if (request.op === 'ext.hello') {
              if (extensionVersion && extensionVersion !== result.version) { end('error'); return; }
              extensionVersion = result.version;
            }
            settle(entry, null, result);
          },
          (error) => {
            if (!current(entry)) return;
            if (!(ext ? extension.EXT_ERRORS : protocol.NATIVE_ERRORS).includes(error?.code)) {
              end("error");
              return;
            }
            settle(entry, error.code);
          },
        );
    });
  });
  const navigate = (_event, _url, inPlace, mainFrame) => {
    if (mainFrame && !inPlace) {
      // The initial empty WebContents has no document/native authority to end.
      // Still invalidate any in-flight connection witness before first load.
      if (document !== null || pending.size || lifetimes.size) end("reload");
      else invalidate("stopped");
    }
  };
  contents.on("did-start-navigation", navigate);
  const gone = () => end("error");
  contents.on("render-process-gone", gone);
  function dispose() {
    if (disposed) return;
    end("local-stop");
    disposed = true;
    for (const channel of [CHANNELS.connect, CHANNELS.request])
      ipcMain.removeHandler(channel);
    ipcMain.removeListener(CHANNELS.witness, witness);
    ipcMain.removeListener(CHANNELS.expired, expired);
    contents.removeListener("did-start-navigation", navigate);
    contents.removeListener("render-process-gone", gone);
    contents.removeListener("destroyed", dispose);
  }
  contents.on("destroyed", dispose);
  return { dispose };
}
module.exports = { installNativeBridge, CHANNELS };
