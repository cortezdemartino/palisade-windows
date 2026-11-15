'use strict';
// An isolated, app-owned DTLS peer. No contextBridge or main-world API.
(() => {
  const { ipcRenderer } = require('electron');
  if (!process.isMainFrame || location.href !== 'palisade-controls://app/assist-input.html') return;
  const nonce = process.argv.find(value => value.startsWith('--palisade-input-owner='))?.slice(23);
  if (!/^[a-f0-9]{48}$/.test(nonce ?? '')) return;
  const documentId = crypto.randomUUID(), encoder = new TextEncoder();
  const EVENT = 'palisade:owned-input-event', COMMAND = 'palisade:owned-input-command';
  const FP = /^sha-256 (?:[0-9A-F]{2}:){31}[0-9A-F]{2}$/;
  const channels = new Map();
  const seenChannels = new Set();
  let peer = null, channel = null, helperTransport = null, admitted = false, resourceChannels = [], input = true,
    verified = false, ended = false, started = false, lastMain = performance.now(),
    lastSequence = 0, work = Promise.resolve(), tokens = 240, releaseTokens = 64, rateAt = performance.now();
  const exact = (value, fields) => value && typeof value === 'object' && !Array.isArray(value) &&
    Object.keys(value).sort().join(',') === ['type', 'nonce', 'document', ...fields].sort().join(',');
  function send(message) {
    if (!ended) ipcRenderer.send(EVENT, { ...message, nonce, document: documentId });
  }
  function stop() {
    if (ended) return;
    admitted = false; verified = false;
    send({ type: 'failed' }); ended = true;
    clearInterval(watch);
    for (const value of channels.values()) { try { value.close(); } catch {} }
    channels.clear();
    if (peer) { try { peer.close(); } catch {} }
    channel = null; peer = null;
    ipcRenderer.removeListener(COMMAND, receive);
  }
  async function observed() {
    if (!peer) return { fingerprint: null, channel: false };
    const dtls = peer.sctp?.transport;
    const open = () => [...channels.values()].some(value => value.readyState === 'open');
    if (peer.connectionState !== 'connected' || peer.sctp?.state !== 'connected' || dtls?.state !== 'connected' || !open()) {
      if (verified || admitted) throw Error('Peer lost');
      return { fingerprint: null, channel: false };
    }
    const certificates = dtls.getRemoteCertificates();
    if (certificates.length !== 1 || certificates[0].byteLength > 16384) throw Error('Certificate unavailable');
    const bytes = new Uint8Array(await crypto.subtle.digest('SHA-256', certificates[0]));
    const fingerprint = 'sha-256 ' + [...bytes].map(value => value.toString(16).padStart(2, '0').toUpperCase()).join(':');
    if (ended || !peer || fingerprint !== helperTransport || peer.connectionState !== 'connected' || dtls.state !== 'connected' || !open()) throw Error('Certificate mismatch');
    verified = true;
    return { fingerprint, channel: true };
  }
  async function offer(message) {
    if (!exact(message, ['sdp', 'helperTransport', 'resourceChannels', 'input']) || started || !FP.test(message.helperTransport) ||
        !Array.isArray(message.resourceChannels) || message.resourceChannels.length > 2 ||
        new Set(message.resourceChannels).size !== message.resourceChannels.length || message.resourceChannels.some(value => !['clipboard', 'file'].includes(value)) ||
        typeof message.input !== 'boolean' || (!message.resourceChannels.length && !message.input) ||
        typeof message.sdp !== 'string' || message.sdp.length > 16384) throw Error('Offer refused');
    started = true; helperTransport = message.helperTransport; resourceChannels = message.resourceChannels; input = message.input;
    const certificate = await RTCPeerConnection.generateCertificate({ name: 'ECDSA', namedCurve: 'P-256' });
    if (ended) return;
    const own = certificate.getFingerprints().filter(row => row.algorithm === 'sha-256');
    if (own.length !== 1) throw Error('Own certificate unavailable');
    const fingerprint = 'sha-256 ' + own[0].value.toUpperCase();
    if (!FP.test(fingerprint)) throw Error('Own certificate invalid');
    // No page-supplied ICE servers or credentials. Reviewed relay configuration
    // is a separate integration dependency; authenticated remote ICE is allowed.
    peer = new RTCPeerConnection({ certificates: [certificate], iceServers: [], bundlePolicy: 'max-bundle' });
    peer.onconnectionstatechange = () => {
      if (['disconnected', 'failed', 'closed'].includes(peer?.connectionState)) stop();
    };
    peer.ontrack = stop;
    peer.onicecandidate = event => send({ type: 'candidate', candidate: event.candidate?.candidate ?? '', mid: event.candidate?.sdpMid ?? null });
    peer.ondatachannel = event => {
      const incoming = event.channel;
      const control = incoming.label === 'control';
      if (ended || seenChannels.has(incoming.label) || !(control ? input : resourceChannels.includes(incoming.label)) || incoming.protocol !== '' ||
          !incoming.ordered || incoming.negotiated || incoming.maxRetransmits !== null ||
          incoming.maxPacketLifeTime !== null) { incoming.close(); stop(); return; }
      channels.set(incoming.label, incoming);
      seenChannels.add(incoming.label);
      if (control) channel = incoming;
      incoming.binaryType = 'arraybuffer';
      incoming.onclose = () => {
        if (control) { stop(); return; }
        channels.delete(incoming.label);
        if (verified && channels.size === 0) stop();
      };
      incoming.onerror = stop;
      incoming.onmessage = event => {
        if (ended) return;
        if (!control) {
          const data = event.data;
          if (typeof data === 'string' ? !data || data.length > 16384 || encoder.encode(data).byteLength > 65536 :
              !(data instanceof ArrayBuffer) || data.byteLength < 5 || data.byteLength > 65540) { stop(); return; }
          if (!admitted) return;
          if (!verified || peer?.connectionState !== 'connected' || peer.sctp?.transport?.state !== 'connected') { stop(); return; }
          send({ type: 'resource', lane: incoming.label, data: typeof data === 'string' ? data : new Uint8Array(data) });
          return;
        }
        const time = performance.now();
        releaseTokens = Math.min(64, releaseTokens + Math.max(0, time - rateAt) * 0.032);
        tokens = Math.min(240, tokens + Math.max(0, time - rateAt) * 0.12); rateAt = time;
        if (typeof event.data !== 'string' || !event.data || event.data.length > 16384 ||
            encoder.encode(event.data).byteLength > 16384) { stop(); return; }
        // Reserve bounded release traffic, still in SCTP/main/native order.
        let release = false;
        try { const value = JSON.parse(event.data); release = value.kind === 'release-all' ||
          (['key', 'button', 'input-key', 'input-button'].includes(value.kind) && value.down === false); }
        catch { stop(); return; }
        if (release ? releaseTokens < 1 : tokens < 1) { stop(); return; }
        if (release) releaseTokens--; else tokens--;
        if (!admitted || !input) return; // Bound and discard, never queue before native acceptance.
        if (!verified || peer?.connectionState !== 'connected' || peer.sctp?.transport?.state !== 'connected') { stop(); return; }
        send({ type: 'packet', text: event.data });
      };
    };
    await peer.setRemoteDescription({ type: 'offer', sdp: message.sdp });
    if (ended) return;
    const answer = await peer.createAnswer();
    if (ended) return;
    await peer.setLocalDescription(answer);
    if (!ended) send({ type: 'answer', sdp: peer.localDescription.sdp, fingerprint });
  }
  function receive(_event, message) {
    if (ended || !message || message.nonce !== nonce || message.document !== documentId) return;
    if (message.type === 'heartbeat') {
      if (!exact(message, ['sequence', 'challenge']) || !Number.isInteger(message.sequence) ||
          message.sequence <= lastSequence || !/^[a-f0-9]{32}$/.test(message.challenge)) { stop(); return; }
      lastSequence = message.sequence; lastMain = performance.now();
      void observed().then(value => send({ type: 'pulse', sequence: message.sequence, challenge: message.challenge, ...value })).catch(stop);
    } else if (message.type === 'admit') {
      if (!exact(message, []) || !verified || admitted) { stop(); return; }
      admitted = true;
    } else if (message.type === 'fence-input') {
      if (!exact(message, [])) { stop(); return; }
      input = false; // Leave the association and independent resource channels alive.
    } else if (message.type === 'offer') {
      work = work.then(() => offer(message)).catch(stop);
    } else if (message.type === 'candidate') {
      if (!exact(message, ['id', 'candidate', 'mid']) || !Number.isInteger(message.id) ||
          typeof message.candidate !== 'string' || message.candidate.length > 1024) { stop(); return; }
      work = work.then(async () => {
        if (ended) return;
        if (!peer?.remoteDescription) throw Error('Unbound candidate');
        await peer.addIceCandidate(message.candidate === '' ? null : { candidate: message.candidate, sdpMid: message.mid });
        send({ type: 'ack', id: message.id });
      }).catch(stop);
    } else stop();
  }
  ipcRenderer.on(COMMAND, receive);
  const watch = setInterval(() => { if (performance.now() - lastMain >= 1500) stop(); }, 100);
  window.addEventListener('beforeunload', stop, { once: true });
  window.addEventListener('DOMContentLoaded', () => send({ type: 'ready' }), { once: true });
})();
