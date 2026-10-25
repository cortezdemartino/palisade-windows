'use strict';
const { createHash } = require('node:crypto');
const { finished } = require('node:stream/promises');
function fileMetadata(value) {
  if (!value || Object.keys(value).sort().join(',') !== 'name,sha256,size' ||
      typeof value.name !== 'string' || value.name.length < 1 || value.name.length > 180 ||
      value.name !== value.name.trim() || value.name !== value.name.normalize('NFC') || /[\x00-\x1f\x7f<>:"/\\|?*]/.test(value.name) ||
      /[\u202a-\u202e\u2066-\u2069]/.test(value.name) || /[. ]$/.test(value.name) || /^\.+$/.test(value.name) ||
      /^(con|prn|aux|nul|com[0-9¹²³]|lpt[0-9¹²³])(?:\.|$)/i.test(value.name) ||
      !Number.isSafeInteger(value.size) || value.size < 0 || value.size > 1024 ** 3 ||
      (typeof value.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(value.sha256))) throw new Error('File metadata refused');
  return Object.freeze({ ...value });
}
function clipboardPayload(format, bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.byteLength > 4 * 1024 * 1024) throw new Error('Clipboard size');
  const data = Buffer.from(bytes);
  if (format === 'text/plain') {
    const text = new TextDecoder('utf-8', { fatal: true }).decode(data);
    if (text.includes('\0')) throw new Error('Invalid text');
  } else if (format === 'image/png') {
    if (data.length < 33 || data.subarray(0, 8).toString('hex') !== '89504e470d0a1a0a' || data.toString('ascii', 12, 16) !== 'IHDR' ||
        data.readUInt32BE(16) < 1 || data.readUInt32BE(20) < 1 || data.readUInt32BE(16) * data.readUInt32BE(20) > 16_000_000) throw new Error('Invalid image');
    // This validates a bounded envelope only; OS image decoding remains a separate boundary.
  } else throw new Error('Unsupported clipboard format');
  return { format, bytes: data, digest: createHash('sha256').update(data).digest('hex') };
}
class ClipboardWindow {
  constructor() { this.last = null; this.active = false; }
  enable() { this.active = true; }
  offer(format, bytes) {
    if (!this.active) throw new Error('Clipboard disabled');
    const payload = clipboardPayload(format, bytes);
    if (this.last === payload.digest + format) return null;
    this.last = payload.digest + format; return payload;
  }
  revoke() { this.active = false; this.last = null; }
}
// Bounded stream receiver, not a network protocol or filesystem/crypto authority.
// Caller supplies an accepted, session-authorized stream and a local quarantine Writable.
async function receiveFile({ metadata, chunks, destination, signal, authorize, onProgress = () => {} }) {
  const file = fileMetadata(metadata), hash = createHash('sha256');
  let total = 0;
  let iterator, rejectAbort, complete = false;
  const interrupted = new Promise((_resolve, reject) => { rejectAbort = reject; });
  // Cancellation can happen synchronously inside an authorization callback.
  // Its original error still propagates through the operation below.
  void interrupted.catch(() => {});
  const completed = finished(destination, { readable: false, cleanup: true });
  void completed.catch(() => rejectAbort(new Error('Transfer destination failed')));
  const abort = () => { destination.destroy(); rejectAbort(new Error("Transfer cancelled")); };
  signal.addEventListener("abort", abort, { once: true });
  function requireAuthorization() {
    if (signal.aborted) throw new Error('Transfer cancelled');
    if (authorize() !== true) throw new Error('Transfer not authorized');
    if (signal.aborted) throw new Error('Transfer cancelled');
  }
  try {
    requireAuthorization();
    if (destination.destroyed || destination.writableEnded) throw new Error('Transfer destination closed');
    iterator = (chunks[Symbol.asyncIterator] || chunks[Symbol.iterator]).call(chunks);
    while (true) {
      requireAuthorization();
      const next = await Promise.race([Promise.resolve(iterator.next()), interrupted]);
      if (next.done) { complete = true; break; }
      const chunk = next.value;
      requireAuthorization();
      if (!(chunk instanceof Uint8Array) || chunk.length === 0 || chunk.length > 65536 || total + chunk.length > file.size) throw new Error('Invalid file chunk');
      const bytes = Buffer.from(chunk);
      total += bytes.length; hash.update(bytes);
      await Promise.race([new Promise((resolve, reject) => destination.write(bytes, error => error ? reject(error) : resolve())), interrupted]);
      onProgress({ received: total, size: file.size });
    }
    requireAuthorization();
    if (total !== file.size || hash.digest('hex') !== file.sha256) throw new Error('File integrity failed');
    destination.end(); await Promise.race([completed, interrupted]);
    requireAuthorization();
    return { size: total, sha256: file.sha256 };
  } catch (error) { destination.destroy(); throw error; }
  finally {
    signal.removeEventListener('abort', abort);
    if (!complete && iterator?.return) void Promise.resolve().then(() => iterator.return()).catch(() => {});
  }
}
module.exports = { fileMetadata, clipboardPayload, ClipboardWindow, receiveFile };
