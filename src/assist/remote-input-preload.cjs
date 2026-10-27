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
  let peer = null, channel = null, helperTransport = null, admitted = false,
    verified = false, ended = false, started = false, lastMain = performance.now(),
    lastSequence = 0, work = Promise.resolve(), tokens = 240, rateAt = performance.now();
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
    if (channel) { try { channel.close(); } catch {} }
    if (peer) { try { peer.close(); } catch {} }
    channel = null; peer = null;
    ipcRenderer.removeListener(COMMAND, receive);
  }
  async function observed() {
    if (!peer) return { fingerprint: null, channel: false };
    const dtls = peer.sctp?.transport;
    if (peer.connectionState !== 'connected' || peer.sctp?.state !== 'connected' || dtls?.state !== 'connected' || channel?.readyState !== 'open') {
      if (verified || admitted) throw Error('Peer lost');
      return { fingerprint: null, channel: false };
    }
    const certificates = dtls.getRemoteCertificates();
    if (certificates.length !== 1 || certificates[0].byteLength > 16384) throw Error('Certificate unavailable');
    const bytes = new Uint8Array(await crypto.subtle.digest('SHA-256', certificates[0]));
    const fingerprint = 'sha-256 ' + [...bytes].map(value => value.toString(16).padStart(2, '0').toUpperCase()).join(':');
    if (ended || !peer || fingerprint !== helperTransport || peer.connectionState !== 'connected' || dtls.state !== 'connected' || channel?.readyState !== 'open') throw Error('Certificate mismatch');
    verified = true;
    return { fingerprint, channel: true };
  }
  async function offer(message) {
    if (!exact(message, ['sdp', 'helperTransport']) || started || !FP.test(message.helperTransport) ||
        typeof message.sdp !== 'string' || message.sdp.length > 16384) throw Error('Offer refused');
    started = true; helperTransport = message.helperTransport;
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
      if (ended || channel || incoming.label !== 'control' || incoming.protocol !== '' ||
          !incoming.ordered || incoming.negotiated || incoming.maxRetransmits !== null ||
          incoming.maxPacketLifeTime !== null) { incoming.close(); stop(); return; }
      channel = incoming; incoming.binaryType = 'arraybuffer';
      incoming.onclose = stop; incoming.onerror = stop;
      incoming.onmessage = event => {
        if (ended) return;
        const time = performance.now();
        tokens = Math.min(240, tokens + Math.max(0, time - rateAt) * 0.12); rateAt = time;
        if (tokens < 1 || typeof event.data !== 'string' || !event.data || event.data.length > 2048 ||
            encoder.encode(event.data).byteLength > 2048) { stop(); return; }
        tokens--;
        if (!admitted) return; // Bound and discard, never queue before native acceptance.
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
