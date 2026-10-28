'use strict';
// Trusted main only. No production startup or renderer imports this module.
// This revision negotiates CHANNEL-ONLY cleanup; it cannot own active input.
const { spawn } = require('node:child_process');
const { randomBytes } = require('node:crypto');
const { TextDecoder } = require('node:util');
const { verifyGuardian } = require('./guardian.cjs');
const PROTOCOL = 'palisade.assist.live', MAX_FRAME = 65_536, CAPACITY = 8;
const decoder = new TextDecoder('utf-8', { fatal: true });
const exact = (v, fields) => v && typeof v === 'object' && !Array.isArray(v) &&
  Object.keys(v).sort().join(',') === fields.sort().join(',');
const unavailable = () => Object.assign(Error('Assist native channel unavailable'), { code: 'assist-native-unavailable' });
const incomplete = () => Object.assign(Error('Assist native cleanup unproven'), { code: 'assist-cleanup-incomplete' });

async function openLiveChannel({ binary, manifestFile, lifecycle, probe = false, signal } = {}) {
  if (!lifecycle || typeof lifecycle.hold !== 'function' || typeof probe !== 'boolean' ||
      (signal && (typeof signal.addEventListener !== 'function' || typeof signal.removeEventListener !== 'function'))) throw unavailable();
  let child = null, launched = false, spawning = true, revoked = false, failed = false,
    closed = false, writeBlocked = false, terminal = false, sequence = 0, heartbeat;
  let resolveExit, rejectExit;
  const exited = new Promise((resolve, reject) => { resolveExit = resolve; rejectExit = reject; });
  void exited.catch(() => {});
  const abort = new AbortController(), pending = new Map(), challenge = randomBytes(32).toString('base64url');
  let buffered = Buffer.alloc(0);
  const holder = lifecycle.hold({ revoke, wait: () => exited });
  const externalAbort = () => revoke();
  signal?.addEventListener('abort', externalAbort, { once: true });
  // Ownership precedes verification/spawn. If global quit wins, spawning is
  // forbidden; the holder remains until that no-child outcome is established.
  if (holder.signal.aborted || signal?.aborted) revoke();
  void exited.then(() => holder.finish('channel-exited'), () => {}).catch(() => {});
  void exited.finally(() => { clearInterval(heartbeat); signal?.removeEventListener('abort', externalAbort); }).catch(() => {});

  function rejectPending() {
    for (const entry of pending.values()) {
      clearTimeout(entry.timer);
      if (!entry.done) { entry.done = true; entry.reject(unavailable()); }
    }
  }
  function revoke() {
    if (revoked) return;
    revoked = true;
    clearInterval(heartbeat);
    abort.abort();
    rejectPending();
    if (!child || closed) return;
    if (failed || writeBlocked || child.stdin.destroyed) {
      // EOF/watchdog performs native revocation even when a write cannot drain.
      // Never kill a cleanup holder or stop draining its output.
      child.stdin.destroy();
      return;
    }
    void send({ type: 'shutdown' }, 0, true).catch(() => {});
  }
  function poison() {
    failed = true;
    if (!revoked) revoke();
    else child?.stdin.destroy();
  }
  function send(command, timeoutMs = 2500, safety = false) {
    if (closed || !child || (!safety && revoked) || sequence >= 0xffffffff || writeBlocked ||
        (!safety && [...pending.values()].filter(v => !['pulse', 'shutdown'].includes(v.type)).length >= CAPACITY)) return Promise.reject(unavailable());
    const bytes = Buffer.from(JSON.stringify({ sequence, command }));
    if (!bytes.length || bytes.length > MAX_FRAME) return Promise.reject(unavailable());
    const id = sequence++, prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length);
    let resolve, reject;
    const answer = new Promise((done, fail) => { resolve = done; reject = fail; });
    const entry = { type: command.type, resolve, reject, done: false, timer: null };
    pending.set(id, entry);
    if (timeoutMs) entry.timer = setTimeout(() => {
      if (entry.done) return;
      entry.done = true; entry.reject(unavailable()); revoke();
    }, timeoutMs);
    try {
      if (!child.stdin.write(Buffer.concat([prefix, bytes]))) writeBlocked = true;
    } catch { poison(); }
    return answer;
  }
  function validate(entry, result) {
    if (entry.type === 'hello') return exact(result, ['type', 'protocol', 'version', 'challenge', 'mode', 'nativeInput', 'cleanupScope']) &&
      result.type === 'hello' && result.protocol === PROTOCOL && result.version === 1 && result.challenge === challenge &&
      result.mode === (probe ? 'live-probe' : 'live') && result.nativeInput === false && result.cleanupScope === 'channel-only';
    if (entry.type === 'status') return exact(result, ['type', 'state', 'authority']) && result.type === 'status' &&
      ['inactive', 'revoked'].includes(result.state) && result.authority === 'none';
    if (entry.type === 'shutdown') return exact(result, ['type', 'challenge', 'scope']) && result.type === 'cleanup-complete' &&
      result.challenge === challenge && result.scope === 'channel-only';
    return ['pulse', 'probe-wait'].includes(entry.type) && exact(result, ['type']) && result.type === entry.type;
  }
  function accept(bytes) {
    let message;
    try {
      const text = decoder.decode(bytes);
      message = JSON.parse(text);
      // The closed native reply vocabulary is ASCII and emitted compactly by
      // serde. Refuse duplicate fields, ambiguous encodings and trailing data.
      if (JSON.stringify(message) !== text) throw unavailable();
    } catch { poison(); return; }
    if (terminal || !exact(message, ['sequence', 'result']) || !Number.isInteger(message.sequence) ||
        message.sequence < 0 || message.sequence >= 0xffffffff) { poison(); return; }
    const entry = pending.get(message.sequence);
    if (!entry || !validate(entry, message.result)) { poison(); return; }
    pending.delete(message.sequence); clearTimeout(entry.timer);
    if (entry.type === 'shutdown') terminal = true;
    if (!entry.done) { entry.done = true; entry.resolve(message.result); }
  }
  function onData(chunk) {
    if (failed || closed) return; // The stream remains drained, with no retained payloads.
    let offset = 0;
    while (offset < chunk.length && !failed) {
      const needed = buffered.length < 4 ? 4 - buffered.length : buffered.readUInt32LE(0) + 4 - buffered.length;
      const length = Math.min(needed, chunk.length - offset);
      buffered = Buffer.concat([buffered, chunk.subarray(offset, offset + length)]); offset += length;
      if (buffered.length < 4) continue;
      const size = buffered.readUInt32LE(0);
      if (!size || size > MAX_FRAME) { poison(); return; }
      if (buffered.length === size + 4) { const bytes = buffered.subarray(4); buffered = Buffer.alloc(0); accept(bytes); }
    }
  }
  try {
    await verifyGuardian(binary, manifestFile);
    if (revoked || holder.signal.aborted || signal?.aborted) throw unavailable();
    child = spawn(binary, [probe ? '--live-probe' : '--live', String(process.pid)], {
      windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'], shell: false,
    });
    child.once('spawn', () => { launched = true; });
    child.once('exit', () => { if (!revoked) poison(); });
    child.on('error', poison);
    child.stdin.on('error', poison);
    child.stdin.on('drain', () => { writeBlocked = false; });
    child.stdout.on('error', poison);
    child.stderr.on('error', () => {});
    child.stderr.resume(); // Fixed native error codes are not authority or cleanup proof.
    child.stdout.on('data', onData);
    child.once('close', (code, exitSignal) => {
      closed = true;
      clearInterval(heartbeat); abort.abort(); rejectPending();
      // Exit alone is never cleanup proof. A future active-mode scope requires
      // retained authenticated descendant ownership, beyond this channel proof.
      if ((!launched && child.pid == null) ||
          (launched && code === 0 && exitSignal === null && terminal && !failed && buffered.length === 0)) resolveExit();
      else rejectExit(incomplete());
    });
    spawning = false;
    await send({ type: 'hello', protocol: PROTOCOL, version: 1, challenge });
    if (revoked || closed) throw unavailable();
    heartbeat = setInterval(() => {
      if (revoked || closed || [...pending.values()].some(v => v.type === 'pulse')) return;
      void send({ type: 'pulse' }).catch(() => { revoke(); });
    }, 1000);
    heartbeat.unref?.();
    const api = { status: () => send({ type: 'status' }), revoke,
      close: () => holder.finish('channel-close'), wait: () => exited, signal: abort.signal, pid: child.pid };
    if (probe) api.probeWait = milliseconds => Number.isInteger(milliseconds) && milliseconds > 0 && milliseconds <= 10_000
      ? send({ type: 'probe-wait', milliseconds }, 12_000) : Promise.reject(unavailable());
    return Object.freeze(api);
  } catch (error) {
    revoke();
    if (!child && spawning) { spawning = false; closed = true; resolveExit(); }
    throw error;
  }
}
module.exports = { openLiveChannel, PROTOCOL, MAX_FRAME };
