'use strict';
const { randomBytes } = require('node:crypto');
const { Writable } = require('node:stream');
const { fileMetadata, receiveFile } = require('./data-model.cjs');
const validId = value => typeof value === 'string' && /^[A-Za-z0-9_-]{8,128}$/.test(value);
class ChunkQueue {
  constructor() { this.chunks = []; this.waiter = null; this.ended = false; this.error = null; this.bytes = 0; }
  push(bytes) {
    if (this.ended || this.error || !(bytes instanceof Uint8Array) || !bytes.length || bytes.length > 65536 || this.bytes + bytes.length > 2 * 1024 * 1024) throw new Error('File receiver backpressure limit');
    const copy = Buffer.from(bytes);
    if (this.waiter) { const waiter = this.waiter; this.waiter = null; waiter.resolve({ value: copy, done: false }); }
    else { this.chunks.push(copy); this.bytes += copy.length; }
  }
  end() { this.ended = true; if (this.waiter) { this.waiter.resolve({ done: true }); this.waiter = null; } }
  fail(error) { this.error = error; this.chunks = []; this.bytes = 0; if (this.waiter) { this.waiter.reject(error); this.waiter = null; } }
  [Symbol.asyncIterator]() { return this; }
  async next() {
    if (this.error) throw this.error;
    if (this.chunks.length) { const value = this.chunks.shift(); this.bytes -= value.length; return { value, done: false }; }
    if (this.ended) return { done: true };
    if (this.waiter) throw new Error('Concurrent file reader');
    return new Promise((resolve, reject) => { this.waiter = { resolve, reject }; });
  }
  async return() { this.fail(new Error('Transfer cancelled')); return { done: true }; }
}
class FileBroker {
  constructor({ files, authorize, transport, signal, role = 'publisher', sendSignal, receiveSignal, onState = () => {} }) {
    if (!files || typeof authorize !== 'function' || !transport || !signal) throw new Error('Native file authority required');
    Object.assign(this, { files, authorize, transport, signal, onState });
    if (!['publisher', 'helper'].includes(role)) throw new Error('Invalid file role');
    this.sendCap = role === 'publisher' ? 'file_to_guest' : 'file_to_host';
    this.receiveCap = role === 'publisher' ? 'file_to_host' : 'file_to_guest';
    this.sendSignal = sendSignal; this.receiveSignal = receiveSignal;
    this.incoming = null; this.outgoing = null; this.closed = false;
    this.abort = () => this.close(); signal.addEventListener('abort', this.abort, { once: true });
    if (signal.aborted) this.close();
    this.sendRevoked = () => { if (this.outgoing) void this.cancel(this.outgoing.id, 'revoked', false); };
    this.receiveRevoked = () => { if (this.incoming) void this.cancel(this.incoming.id, 'revoked', false); };
    sendSignal?.addEventListener('abort', this.sendRevoked, { once: true });
    receiveSignal?.addEventListener('abort', this.receiveRevoked, { once: true });
  }
  live() { if (this.closed || this.signal.aborted) throw new Error('File session ended'); }
  async allowed(capability) { this.live(); const directional = capability === this.sendCap ? this.sendSignal : this.receiveSignal; if (directional?.aborted || await this.authorize(capability) !== true) throw new Error('File permission denied'); this.live(); if (directional?.aborted) throw new Error('File permission revoked'); }
  publish(state) { try { this.onState(state); } catch {} }
  timer(state) { clearTimeout(state.timer); state.timer = setTimeout(() => { void this.cancel(state.id, 'timeout'); }, 30_000); state.timer.unref?.(); }
  async control(kind, fields) { this.live(); return this.transport.control(kind, fields); }
  // Only an app-owned menu/button can invoke native selection. No page path is accepted.
  async userSend() {
    this.live(); if (this.outgoing) throw new Error('A file is already being sent');
    const id = 'transfer_' + randomBytes(18).toString('base64url');
    let resolveAccepted, rejectAccepted;
    const accepted = new Promise((resolve, reject) => { resolveAccepted = resolve; rejectAccepted = reject; });
    void accepted.catch(() => {});
    const state = { id, handle: null, offset: 0, chunks: 0, accepted: false, resolveAccepted, rejectAccepted, abort: new AbortController() };
    this.outgoing = state;
    try {
      await this.allowed(this.sendCap);
      const selected = await this.files.selectSend({ signal: state.abort.signal });
      if (selected) state.handle = selected.handle;
      this.live(); if (this.outgoing !== state) throw new Error('Transfer ended');
      if (!selected) return;
      const metadata = fileMetadata(selected.metadata); state.metadata = metadata; this.timer(state);
      await this.allowed(this.sendCap); await this.control('file-offer', { transfer: id, ...metadata });
      this.publish({ direction: 'send', state: 'awaiting-acceptance', id, name: metadata.name, size: metadata.size });
      await accepted; await this.allowed(this.sendCap);
      while (state.offset < metadata.size) {
        await this.allowed(this.sendCap);
        const bytes = await this.files.read(state.handle, state.offset);
        this.live(); if (this.outgoing !== state || !(bytes instanceof Uint8Array) || !bytes.length || bytes.length > 65536 || state.offset + bytes.length > metadata.size) throw new Error('Source file changed');
        await this.allowed(this.sendCap); await this.transport.chunk(state.chunks, bytes);
        this.live(); state.offset += bytes.length; state.chunks++; this.timer(state);
        this.publish({ direction: 'send', state: 'sending', id, transferred: state.offset, size: metadata.size });
      }
      await this.allowed(this.sendCap); await this.control('file-complete', { transfer: id, chunks: state.chunks });
      this.publish({ direction: 'send', state: 'sent', id, size: metadata.size });
    } catch (error) {
      this.publish({ direction: 'send', state: 'failed', id });
      if (!this.closed) await this.control('file-cancel', { transfer: id }).catch(() => {});
      throw error;
    } finally {
      clearTimeout(state.timer); state.abort.abort(); if (state.handle) await this.files.cancel(state.handle).catch(() => {});
      if (this.outgoing === state) this.outgoing = null;
    }
  }
  async offer(message) {
    await this.allowed(this.receiveCap);
    if (this.incoming || !validId(message?.transfer) || this.outgoing?.id === message.transfer) throw new Error('Incoming transfer refused');
    const metadata = fileMetadata({ name: message.name, size: message.size, sha256: message.sha256 });
    this.incoming = { id: message.transfer, metadata, handle: null, accepted: false, nextChunk: 0, received: 0, abort: new AbortController() };
    this.timer(this.incoming);
    this.publish({ direction: 'receive', state: 'offer', id: message.transfer, name: metadata.name, size: metadata.size });
  }
  async userAccept(id) {
    await this.allowed(this.receiveCap); const state = this.incoming;
    if (!state || state.id !== id || state.accepted) throw new Error('No pending file');
    state.accepted = true; // Prevent a second save dialog while the first is pending.
    clearTimeout(state.timer);
    let handle;
    try {
      handle = await this.files.selectReceive(state.metadata, { signal: state.abort.signal });
      this.live(); if (!handle || this.incoming !== state) throw new Error('Save cancelled');
      await this.allowed(this.receiveCap); state.handle = handle; state.queue = new ChunkQueue();
      let written = 0;
      const destination = new Writable({ write: (chunk, _encoding, done) => {
        this.files.write(handle, written, chunk).then(() => { written += chunk.length; done(); }, done);
      } });
      state.result = receiveFile({ metadata: state.metadata, chunks: state.queue, destination, signal: state.abort.signal,
        authorize: async () => { await this.allowed(this.receiveCap); return this.incoming === state; },
        onProgress: progress => this.publish({ direction: 'receive', state: 'receiving', id, transferred: progress.received, size: progress.size }) });
      // Handle failure immediately; final completion still awaits this exact result.
      void state.result.catch(() => { void this.cancel(id, 'failed'); });
      await this.control('file-accept', { transfer: id }); this.timer(state);
    } catch (error) {
      if (handle) await this.files.cancel(handle).catch(() => {});
      await this.cancel(id, 'cancelled'); throw error;
    }
  }
  async receiveChunk(index, bytes) {
    const state = this.incoming;
    try {
      await this.allowed(this.receiveCap);
      if (!state?.queue || this.incoming !== state || !state.accepted || index !== state.nextChunk || !(bytes instanceof Uint8Array) || state.received + bytes.length > state.metadata.size) throw new Error('Unexpected file chunk');
      state.queue.push(bytes); state.nextChunk++; state.received += bytes.length; this.timer(state);
    } catch (error) { if (state) await this.cancel(state.id, 'failed'); throw error; }
  }
  async receivedControl(kind, message) {
    this.live(); const id = message?.transfer;
    if (!validId(id)) throw new Error('Invalid transfer');
    if (kind === 'file-offer') return this.offer(message);
    if (kind === 'file-accept') {
      await this.allowed(this.sendCap); const state = this.outgoing;
      if (!state || state.id !== id || state.accepted) throw new Error('Unexpected file acceptance');
      state.accepted = true; state.resolveAccepted(); this.timer(state); return;
    }
    if (kind === 'file-reject' || kind === 'file-cancel') {
      await this.allowed(this.outgoing?.id === id ? this.sendCap : this.receiveCap);
      return this.cancel(id, kind, false);
    }
    if (kind === 'file-complete') {
      await this.allowed(this.receiveCap); const state = this.incoming;
      if (!state?.queue || state.id !== id || !Number.isInteger(message.chunks) || message.chunks !== state.nextChunk) throw new Error('Unexpected completion');
      try {
        state.queue.end(); await state.result; await this.allowed(this.receiveCap);
        if (this.incoming !== state) throw new Error('Transfer ended');
        await this.files.commit(state.handle); this.live(); clearTimeout(state.timer); this.incoming = null;
        this.publish({ direction: 'receive', state: 'complete', id, size: state.metadata.size });
      } catch (error) { await this.cancel(id, 'integrity-failure'); throw error; }
      return;
    }
    throw new Error('Unknown file message');
  }
  async cancel(id, reason = 'cancelled', notify = true) {
    const state = this.incoming?.id === id ? this.incoming : this.outgoing?.id === id ? this.outgoing : null;
    if (!state) return;
    if (this.incoming === state) this.incoming = null;
    if (this.outgoing === state) this.outgoing = null;
    clearTimeout(state.timer); state.abort?.abort(); state.queue?.fail(new Error('Transfer cancelled'));
    state.rejectAccepted?.(new Error('Transfer cancelled'));
    if (state.handle) await this.files.cancel(state.handle).catch(() => {});
    if (notify && !this.closed) await this.control('file-cancel', { transfer: id }).catch(() => {});
    this.publish({ state: reason, id });
  }
  close() {
    if (this.closed) return;
    this.closed = true; this.signal.removeEventListener('abort', this.abort);
    this.sendSignal?.removeEventListener('abort', this.sendRevoked); this.receiveSignal?.removeEventListener('abort', this.receiveRevoked);
    if (this.incoming) void this.cancel(this.incoming.id, 'session-ended', false);
    if (this.outgoing) void this.cancel(this.outgoing.id, 'session-ended', false);
  }
}
module.exports = { FileBroker, ChunkQueue };
