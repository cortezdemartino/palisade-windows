'use strict';
const { BrowserWindow, ipcMain } = require('electron');
const path = require('node:path');
const { randomBytes } = require('node:crypto');
const { installControlsProtocol } = require('../desktop-window.cjs');
const { isPalisadeUrl } = require('../security-policy.cjs');
const { sanitize, pairAllowed, FP } = require('./local-video-policy.cjs');
const { Liveness } = require('./local-video-liveness.cjs');
const { performance } = require('node:perf_hooks');
const PROVIDER = 'palisade-controls://app/assist-provider.html',
  EVENT = 'palisade:local-video-event',
  COMMAND = 'palisade:local-video-command';
const exact = (value, keys) =>
  value &&
  typeof value === 'object' &&
  !Array.isArray(value) &&
  Object.keys(value).sort().join(',') === keys.sort().join(',');
// Internal experimental component. There is no hosted start/signaling surface.
function createLocalVideoHop({ consumer, parent, inspect, enabled = false, onState = () => {} }) {
  if (typeof inspect !== 'function') throw Error('Native ownership verifier required');
  let consumerDocument = null,
    active = null,
    disposed = false,
    queue = Promise.resolve(),
    readyResolve,
    epoch = 0,
    queued = 0;
  const ready = new Promise((resolve) => {
    readyResolve = resolve;
  });
  const trustedConsumer = (event) =>
    event.sender === consumer &&
    !consumer.isDestroyed() &&
    event.senderFrame === consumer.mainFrame &&
    isPalisadeUrl(consumer.getURL()) &&
    isPalisadeUrl(event.senderFrame?.url);
  const publish = (state, operation = active?.publicOperation ?? null) => {
    try {
      onState(state, operation);
    } catch {}
  };
  const send = (contents, document, message) => {
    if (!contents.isDestroyed())
      contents.send(COMMAND, { ...message, document, operation: active?.nonce });
  };
  function stop() {
    epoch++;
    const state = active;
    if (!state) return;
    active = null;
    state.abort.abort();
    clearTimeout(state.timeout);
    clearInterval(state.watch);
    if (!consumer.isDestroyed())
      consumer.send(COMMAND, {
        type: 'stop',
        document: state.consumerDocument,
        operation: state.nonce,
      });
    if (!state.provider.isDestroyed()) state.provider.destroy();
    state.reject(new Error('Local capture ended'));
    publish('ended', state.publicOperation);
  }
  async function freshBindings(state, local, remote) {
    const ports = [...new Set([...local.keys(), ...remote.keys()])];
    const response = await inspect(ports);
    if (active !== state || state.abort.signal.aborted) throw Error('Stopped');
    if (
      !exact(response, ['candidates']) ||
      !Array.isArray(response.candidates) ||
      response.candidates.length !== ports.length
    )
      throw Error('Unowned route');
    const current = new Map(response.candidates.map((row) => [row.port, row.address]));
    if (
      current.size !== ports.length ||
      ports.some((port) => current.get(port) !== (local.get(port) ?? remote.get(port)))
    )
      throw Error('Route changed');
  }
  async function handle(event, message, arrived) {
    const isConsumer = trustedConsumer(event),
      state = active;
    const isProvider =
      state &&
      !state.provider.isDestroyed() &&
      event.sender === state.provider.webContents &&
      event.senderFrame === state.provider.webContents.mainFrame &&
      event.senderFrame?.url === PROVIDER &&
      event.sender.getURL() === PROVIDER;
    if (!isConsumer && !isProvider) return;
    if (
      !message ||
      typeof message.document !== 'string' ||
      !/^[a-f0-9-]{36}$/.test(message.document)
    )
      throw Error('Invalid document');
    if (message.type === 'ready') {
      if (!exact(message, ['type', 'document', 'operation', 'role']) || message.operation !== null)
        throw Error('Invalid ready');
      if (isConsumer && message.role === 'consumer') {
        if (active && consumerDocument !== message.document) stop();
        consumerDocument = message.document;
        readyResolve();
        return;
      }
      if (isProvider && message.role === 'provider' && !state.providerDocument) {
        state.providerDocument = message.document;
        state.mediaArmed = true;
        send(state.provider.webContents, message.document, { type: 'start', source: state.source });
        return;
      }
      throw Error('Unexpected ready');
    }
    if (
      !state ||
      message.operation !== state.nonce ||
      message.document !== (isConsumer ? state.consumerDocument : state.providerDocument)
    )
      return;
    const role = isConsumer ? 'consumer' : 'provider';
    if (message.type === 'failed' || message.type === 'stopped') {
      stop();
      return;
    }
    if (message.type === 'alive') {
      if (
        !exact(message, ['type', 'document', 'operation', 'sequence', 'challenge']) ||
        state.phase === 'exposed'
      )
        throw Error('Invalid heartbeat');
      const pending = state.liveness.match(role, message, arrived);
      if (!pending) return;
      state.liveness.finish(role, pending, false);
      return;
    }
    if (message.type === 'offer' || message.type === 'answer') {
      if (
        !exact(message, ['type', 'document', 'operation', 'sdp', 'fingerprint']) ||
        !FP.test(message.fingerprint)
      )
        throw Error('Invalid signaling');
      if (message.type === 'offer' && role === 'provider' && state.phase === 'starting') {
        state.mediaArmed = false;
        const offer = await sanitize(message.sdp, {
          direction: 'a=sendonly',
          setup: 'a=setup:actpass',
          inspect,
          signal: state.abort.signal,
        });
        if (active !== state || offer.fingerprint !== message.fingerprint)
          throw Error('Unowned certificate');
        state.providerOffer = offer;
        state.phase = 'offered';
        send(consumer, state.consumerDocument, {
          type: 'offer',
          sdp: offer.sdp,
          fingerprint: offer.fingerprint,
        });
        return;
      }
      if (message.type === 'answer' && role === 'consumer' && state.phase === 'offered') {
        const answer = await sanitize(message.sdp, {
          direction: 'a=recvonly',
          setup: 'a=setup:active',
          inspect,
          signal: state.abort.signal,
        });
        if (active !== state || answer.fingerprint !== message.fingerprint)
          throw Error('Unowned certificate');
        state.consumerAnswer = answer;
        state.phase = 'answered';
        send(state.provider.webContents, state.providerDocument, {
          type: 'answer',
          sdp: answer.sdp,
          fingerprint: answer.fingerprint,
        });
        return;
      }
      throw Error('Unexpected signaling');
    }
    if (message.type === 'connected') {
      if (
        !exact(message, [
          'type',
          'document',
          'operation',
          'peerFingerprint',
          'pair',
          'sequence',
          'challenge',
        ]) ||
        !['answered', 'exposing', 'exposed'].includes(state.phase)
      )
        throw Error('Invalid route report');
      const pending = state.liveness.match(role, message, arrived);
      if (!pending) return;
      const local = role === 'consumer' ? state.consumerAnswer : state.providerOffer,
        remote = role === 'consumer' ? state.providerOffer : state.consumerAnswer;
      if (
        message.peerFingerprint !== remote.fingerprint ||
        !pairAllowed(message.pair, local.bindings, remote.bindings)
      )
        throw Error('Route changed');
      await freshBindings(state, local.bindings, remote.bindings);
      if (state.liveness.match(role, message, arrived) !== pending)
        throw Error('Stale route report');
      const previous = state[role + 'Report'];
      if (previous && JSON.stringify(previous.pair) !== JSON.stringify(message.pair))
        throw Error('Route changed');
      state.liveness.finish(role, pending, true);
      state[role + 'Report'] = { pair: message.pair };
      if (state.phase === 'answered' && state.consumerReport && state.providerReport) {
        state.phase = 'exposing';
        send(consumer, state.consumerDocument, {
          type: 'expose',
          fingerprint: state.providerOffer.fingerprint,
          pair: state.consumerReport.pair,
          publicOperation: state.publicOperation,
        });
      }
      return;
    }
    if (message.type === 'exposed' && role === 'consumer' && state.phase === 'exposing') {
      if (
        !exact(message, ['type', 'document', 'operation', 'elementId', 'publicOperation']) ||
        message.elementId !== 'palisade-capture-' + state.nonce ||
        message.publicOperation !== state.publicOperation
      )
        throw Error('Invalid stream');
      clearTimeout(state.timeout);
      state.phase = 'exposed';
      state.resolve({ operation: state.nonce, elementId: message.elementId });
      publish('capturing');
      return;
    }
    throw Error('Invalid transition');
  }
  const receive = (event, message) => {
    const state = active,
      fromProvider =
        state &&
        !state.provider.isDestroyed() &&
        event.sender === state.provider.webContents &&
        event.senderFrame === state.provider.webContents.mainFrame &&
        event.senderFrame?.url === PROVIDER;
    if (!trustedConsumer(event) && !fromProvider) return;
    if (queued >= 8) {
      stop();
      return;
    }
    queued++;
    const arrived = performance.now();
    queue = queue
      .then(() => handle(event, message, arrived))
      .catch(() => {
        if (active === state) {
          publish('unavailable');
          stop();
        }
      })
      .finally(() => {
        queued--;
      });
  };
  ipcMain.on(EVENT, receive);
  const navigation = (_event, _url, inPlace, mainFrame) => {
    if (mainFrame && !inPlace) {
      consumerDocument = null;
      stop();
    }
  };
  consumer.on('did-start-navigation', navigation);
  consumer.on('render-process-gone', stop);
  consumer.on('destroyed', stop);
  parent?.on('hide', stop);
  parent?.on('minimize', stop);
  parent?.on('closed', stop);
  async function start(source, { publicOperation = null } = {}) {
    const requestedEpoch = epoch;
    if (
      enabled !== true ||
      disposed ||
      active ||
      consumer.isDestroyed() ||
      !isPalisadeUrl(consumer.getURL())
    )
      throw Error('Local capture unavailable');
    if (publicOperation !== null && (typeof publicOperation !== 'string' || !/^[A-Za-z0-9_-]{8,128}$/.test(publicOperation)))
      throw Error('Invalid capture publication');
    if (
      !exact(source, source?.kind === 'synthetic' ? ['kind'] : ['kind', 'id']) ||
      !['synthetic', 'desktop'].includes(source.kind) ||
      (source.kind === 'desktop' && !/^(screen|window):\d+:\d+$/.test(source.id))
    )
      throw Error('Invalid selected source');
    if (
      source.kind === 'desktop' &&
      (!parent || parent.isDestroyed() || !parent.isVisible() || parent.isMinimized())
    )
      throw Error('Local capture requires a visible application');
    if (!consumerDocument) {
      let timer;
      try {
        await Promise.race([
          ready,
          new Promise((_resolve, reject) => {
            timer = setTimeout(() => reject(Error('Consumer unavailable')), 5000);
          }),
        ]);
      } finally {
        clearTimeout(timer);
      }
    }
    if (!consumerDocument || disposed || active || epoch !== requestedEpoch)
      throw Error('Consumer unavailable');
    const nonce = randomBytes(24).toString('hex'),
      abort = new AbortController();
    const provider = new BrowserWindow({
      show: false,
      title: 'Palisade capture provider',
      webPreferences: {
        partition: 'palisade-capture-' + nonce,
        preload: path.join(__dirname, 'local-video-preload.cjs'),
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        nodeIntegrationInWorker: false,
        webSecurity: true,
        webviewTag: false,
        devTools: false,
        backgroundThrottling: false,
      },
    });
    const profile = provider.webContents.session;
    installControlsProtocol(profile);
    provider.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
    provider.webContents.on('will-navigate', (event) => event.preventDefault());
    provider.webContents.on('will-redirect', (event) => event.preventDefault());
    profile.webRequest.onBeforeRequest({ urls: ['*://*/*'] }, (details, done) =>
      done({ cancel: details.url !== PROVIDER }),
    );
    const expectedOrigin = (value) =>
      ['palisade-controls://app', 'palisade-controls://app/', PROVIDER].includes(value);
    const owned = (contents, details) =>
      active?.provider === provider &&
      active.mediaArmed &&
      contents === provider.webContents &&
      !contents.isDestroyed() &&
      contents.getURL() === PROVIDER &&
      details?.isMainFrame === true;
    profile.setPermissionRequestHandler((contents, permission, done, details) =>
      done(
        permission === 'media' &&
          owned(contents, details) &&
          expectedOrigin(details.requestingUrl) &&
          Array.isArray(details.mediaTypes) &&
          details.mediaTypes.every((type) => type === 'video'),
      ),
    );
    profile.setPermissionCheckHandler(
      (contents, permission, origin, details) =>
        permission === 'media' &&
        owned(contents, details) &&
        expectedOrigin(origin) &&
        !['audio', 'camera'].includes(details.mediaType),
    );
    profile.setDisplayMediaRequestHandler((_request, done) => done(null), {
      useSystemPicker: false,
    });
    profile.setDevicePermissionHandler(() => false);
    const result = new Promise((resolve, reject) => {
      const state = {
        nonce,
        publicOperation,
        source,
        provider,
        consumerDocument,
        providerDocument: null,
        mediaArmed: false,
        abort,
        resolve,
        reject,
        phase: 'starting',
        liveness: new Liveness(),
      };
      active = state;
      state.timeout = setTimeout(() => { if (active === state) stop(); }, 15000);
      state.timeout.unref?.();
      state.watch = setInterval(() => {
        if (active !== state) return;
        for (const role of ['provider', 'consumer']) {
          const document = state[role + 'Document'];
          if (!document) continue;
          if (state.liveness.expired(role)) {
            stop();
            return;
          }
          const challenge = state.liveness.issue(role);
          if (challenge)
            send(role === 'provider' ? state.provider.webContents : consumer, document, {
              type: 'heartbeat',
              sequence: challenge.sequence,
              challenge: challenge.challenge,
            });
        }
      }, 250);
      state.watch.unref?.();
    });
    provider.on('closed', () => {
      if (active?.provider === provider) stop();
    });
    const lostProvider = () => { if (active?.provider === provider) stop(); };
    provider.webContents.on('render-process-gone', lostProvider);
    provider.webContents.on('did-fail-load', lostProvider);
    publish('starting');
    void provider.loadURL(PROVIDER).catch(lostProvider);
    return result;
  }
  function dispose() {
    if (disposed) return;
    disposed = true;
    stop();
    ipcMain.removeListener(EVENT, receive);
    consumer.removeListener('did-start-navigation', navigation);
    consumer.removeListener('render-process-gone', stop);
    consumer.removeListener('destroyed', stop);
    for (const type of ['hide', 'minimize', 'closed']) parent?.removeListener(type, stop);
  }
  return { start, stop, dispose, busy: () => Boolean(active) };
}
module.exports = { createLocalVideoHop, PROVIDER };
