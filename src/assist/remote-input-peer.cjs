'use strict';
// Trusted-main internal transport. No production caller or hosted-page surface.
// The future coordinator supplies the independently verified grant binding and
// revokes native authority synchronously in onClosed, before awaiting cleanup.
const { BrowserWindow, ipcMain } = require('electron');
const path = require('node:path');
const { randomBytes } = require('node:crypto');
const { performance } = require('node:perf_hooks');
const { installControlsProtocol } = require('../desktop-window.cjs');
const { Liveness } = require('./local-video-liveness.cjs');
const { FP, TOKEN, dataSdp, candidate, inputPacket } = require('./remote-input-policy.cjs');
const URL = 'palisade-controls://app/assist-input.html';
const EVENT = 'palisade:owned-input-event', COMMAND = 'palisade:owned-input-command';
const exact = (value, fields) => value && typeof value === 'object' && !Array.isArray(value) &&
  Object.keys(value).sort().join(',') === [...fields].sort().join(',');
async function createOwnedInputPeer({ enabled = false, session, helperTransport, signal,
  onObserved, onPacket, onCandidate, onClosed } = {}) {
  if (enabled !== true) throw Error('Owned input peer disabled');
  if (typeof session !== 'string' || typeof helperTransport !== 'string' ||
      !TOKEN.test(session) || !FP.test(helperTransport) || signal?.aborted ||
      [onObserved, onPacket, onCandidate, onClosed].some(fn => typeof fn !== 'function'))
    throw Error('Native input coordinator required');
  const nonce = randomBytes(24).toString('hex'), abort = new AbortController();
  const window = new BrowserWindow({ show: false, title: 'Palisade Assist input transport',
    webPreferences: { partition: 'palisade-input-' + nonce,
      preload: path.join(__dirname, 'remote-input-preload.cjs'),
      additionalArguments: ['--palisade-input-owner=' + nonce], sandbox: true,
      contextIsolation: true, nodeIntegration: false, nodeIntegrationInWorker: false,
      webSecurity: true, allowRunningInsecureContent: false, webviewTag: false,
      devTools: false, backgroundThrottling: false } });
  const contents = window.webContents, profile = contents.session;
  installControlsProtocol(profile);
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
  profile.setPermissionRequestHandler((_contents, _permission, done) => done(false));
  profile.setPermissionCheckHandler(() => false);
  profile.setDevicePermissionHandler(() => false);
  profile.setDisplayMediaRequestHandler((_request, done) => done(null), { useSystemPicker: false });
  profile.webRequest.onBeforeRequest({ urls: ['*://*/*'] }, (details, done) => done({ cancel: details.url !== URL }));
  let document = null, ended = false, offered = false, answer = null, info = null,
    observed = false, verifying = false, requestId = 0, queued = 0, tail = Promise.resolve(),
    tokens = 240, rateAt = performance.now(), candidateCount = 0;
  const pending = new Map(), remoteCandidates = new Set(), liveness = new Liveness();
  let bootResolve, bootReject, finishedResolve, finishedReject, connectTimer;
  const boot = new Promise((resolve, reject) => { bootResolve = resolve; bootReject = reject; });
  const finished = new Promise((resolve, reject) => { finishedResolve = resolve; finishedReject = reject; });
  // Retain rejection for the coordinator without an unhandled-rejection crash.
  void finished.catch(() => {});
  const send = message => {
    if (!ended && document && !contents.isDestroyed()) contents.send(COMMAND, { ...message, nonce, document });
  };
  function close(reason = 'stopped') {
    if (ended) return finished;
    ended = true;
    abort.abort();
    clearTimeout(startTimer); clearTimeout(connectTimer); clearInterval(watch);
    signal?.removeEventListener('abort', cancel);
    ipcMain.removeListener(EVENT, receive);
    const failure = Error('Owned input peer ended');
    bootReject(failure);
    if (answer) { answer.reject(failure); answer = null; }
    for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(failure); }
    pending.clear();
    // This callback must invalidate native session/input flags immediately.
    // Its returned cleanup promise is retained; no input child is killed here.
    try { Promise.resolve(onClosed(reason)).then(finishedResolve, finishedReject); }
    catch (error) { finishedReject(error); }
    if (!window.isDestroyed()) window.destroy();
    return finished;
  }
  const cancel = () => { void close('cancelled'); };
  function schema(message, fields) { return exact(message, ['type', 'nonce', 'document', ...fields]); }
  const trusted = event => !ended && !contents.isDestroyed() && event.sender === contents &&
    event.senderFrame === contents.mainFrame && event.senderFrame?.url === URL && contents.getURL() === URL;
  function receive(event, message) {
    if (!trusted(event)) return;
    if (!message || message.nonce !== nonce || typeof message.document !== 'string' ||
        !/^[a-f0-9-]{36}$/.test(message.document)) { void close('invalid-message'); return; }
    if (message.type === 'ready') {
      if (!schema(message, []) || document) { void close('invalid-message'); return; }
      document = message.document; clearTimeout(startTimer); bootResolve(); return;
    }
    if (message.document !== document) return;
    if (liveness.expired('peer')) { void close('transport-timeout'); return; }
    try {
      if (message.type === 'answer') {
        if (!schema(message, ['sdp', 'fingerprint']) || !answer || !FP.test(message.fingerprint)) throw Error('answer');
        const own = dataSdp(message.sdp, 'answer', message.fingerprint);
        if (own.mid !== info.mid) throw Error('mid');
        const reply = answer; answer = null;
        reply.resolve(Object.freeze({ sdp: message.sdp, fingerprint: own.fingerprint }));
      } else if (message.type === 'candidate') {
        if (!schema(message, ['candidate', 'mid']) || !offered || ++candidateCount > 65) throw Error('candidate');
        candidate(message.candidate, message.mid, info.mid);
        Promise.resolve(onCandidate({ candidate: message.candidate, mid: message.mid }))
          .catch(() => { void close('signaling-failed'); });
      } else if (message.type === 'ack') {
        if (!schema(message, ['id']) || !pending.has(message.id)) throw Error('ack');
        const entry = pending.get(message.id); pending.delete(message.id);
        clearTimeout(entry.timer); entry.resolve();
      } else if (message.type === 'pulse') {
        if (!schema(message, ['sequence', 'challenge', 'fingerprint', 'channel']) ||
            typeof message.channel !== 'boolean' || (message.fingerprint !== null && message.fingerprint !== helperTransport)) throw Error('peer');
        const challenge = liveness.match('peer', message, performance.now());
        if (!challenge) return;
        liveness.finish('peer', challenge, true);
        if (observed && (!message.channel || message.fingerprint !== helperTransport)) throw Error('lost-peer');
        if (!observed && !verifying && message.channel && message.fingerprint === helperTransport) {
          if (!offered || answer) throw Error('unanswered');
          verifying = true;
          Promise.resolve(onObserved(message.fingerprint, abort.signal)).then(accepted => {
            if (ended) return;
            if (liveness.expired('peer')) { void close('transport-timeout'); return; }
            if (accepted !== true) { void close('native-refused'); return; }
            observed = true; clearTimeout(connectTimer); send({ type: 'admit' });
          }).catch(() => { void close('native-refused'); });
        }
      } else if (message.type === 'packet') {
        if (!schema(message, ['text']) || !observed) throw Error('unverified-packet');
        const text = inputPacket(message.text, session), arrived = performance.now();
        tokens = Math.min(240, tokens + Math.max(0, arrived - rateAt) * 0.12); rateAt = arrived;
        if (tokens < 1 || queued >= 8) throw Error('flood');
        tokens--; queued++;
        tail = tail.then(async () => {
          if (ended) return;
          if (liveness.expired('peer')) throw Error('expired-peer');
          if (performance.now() - arrived >= 100) throw Error('stale-packet');
          await onPacket(text, abort.signal);
        }).catch(() => { void close('input-refused'); }).finally(() => { queued--; });
      } else if (message.type === 'failed') {
        if (!schema(message, [])) throw Error('failed');
        void close('transport-failed');
      } else throw Error('message');
    } catch { void close('invalid-transport'); }
  }
  const startTimer = setTimeout(() => { void close('startup-timeout'); }, 5000);
  const watch = setInterval(() => {
    if (!document || ended) return;
    if (liveness.expired('peer')) { void close('transport-timeout'); return; }
    const value = liveness.issue('peer');
    if (value) send({ type: 'heartbeat', sequence: value.sequence, challenge: value.challenge });
  }, 250);
  startTimer.unref?.(); watch.unref?.();
  ipcMain.on(EVENT, receive);
  signal?.addEventListener('abort', cancel, { once: true });
  contents.on('will-navigate', event => { event.preventDefault(); void close('navigation'); });
  contents.on('will-redirect', event => { event.preventDefault(); void close('navigation'); });
  contents.on('render-process-gone', () => { void close('renderer-ended'); });
  contents.on('did-fail-load', () => { void close('load-failed'); });
  contents.on('did-start-navigation', (_event, _url, inPlace, mainFrame) => {
    if (document && mainFrame && !inPlace) void close('navigation');
  });
  window.on('closed', () => { void close('window-ended'); });
  void window.loadURL(URL).catch(() => { void close('load-failed'); });
  await boot;
  return Object.freeze({
    async offer(sdp) {
      if (ended || offered) throw Error('Stale input offer');
      try { info = dataSdp(sdp, 'offer', helperTransport); }
      catch { void close('invalid-offer'); throw Error('Invalid input offer'); }
      offered = true;
      for (const value of info.candidates) remoteCandidates.add(value);
      connectTimer = setTimeout(() => { void close('connection-timeout'); }, 15000);
      connectTimer.unref?.();
      return new Promise((resolve, reject) => {
        answer = { resolve, reject };
        send({ type: 'offer', sdp, helperTransport });
      });
    },
    async candidate(value, mid) {
      if (ended || !offered) throw Error('Stale input candidate');
      try { candidate(value, mid, info.mid); }
      catch { void close('invalid-candidate'); throw Error('Invalid input candidate'); }
      if (remoteCandidates.has(value)) return;
      if (remoteCandidates.size >= 64 || pending.size >= 8 || requestId >= 0xffffffff) {
        void close('candidate-limit'); throw Error('Input candidate limit');
      }
      remoteCandidates.add(value);
      const id = ++requestId;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => { void close('candidate-timeout'); }, 5000);
        timer.unref?.(); pending.set(id, { resolve, reject, timer });
        send({ type: 'candidate', id, candidate: value, mid: mid ?? info.mid });
      });
    },
    close,
    finished,
    verified: () => observed && !ended && !liveness.expired('peer'),
  });
}
module.exports = { createOwnedInputPeer };
