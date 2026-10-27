'use strict';
// App-owned peers only. No contextBridge, signaling API or certificate is exposed.
(() => {
  const { ipcRenderer } = require('electron');
  if (!process.isMainFrame) return;
  const provider = location.href === 'palisade-controls://app/assist-provider.html';
  if (!provider && location.origin !== 'https://palisade.onyxops.org') return;
  const documentId = crypto.randomUUID(),
    command = 'palisade:local-video-command',
    channel = 'palisade:local-video-event';
  let current = null;
  const retired = new Set();
  function createPeer(operation) {
    let pc = null,
      track = null,
      node = null,
      expectedPeer = null,
      ownFingerprint = null,
      timer = null,
      drawTimer = null,
      reporting = false,
      exposed = false,
      ended = false,
      approvedPair = null,
      lastHeartbeat = performance.now(),
      stage = 'idle',
      pendingHeartbeat = null,
      lastSequence = 0;
    const heartbeatTimer = setInterval(() => {
      if (operation && !ended && performance.now() - lastHeartbeat > 2000) stop();
    }, 250);
    const emit = (type, fields = {}) =>
      ipcRenderer.send(channel, { type, document: documentId, operation, ...fields });
    function stop(notify = true) {
      if (ended) return;
      ended = true;
      clearInterval(timer);
      clearInterval(drawTimer);
      clearInterval(heartbeatTimer);
      timer = null;
      if (pc) {
        pc.onconnectionstatechange = null;
        pc.ontrack = null;
        pc.close();
      }
      track?.stop();
      if (node) {
        node.srcObject = null;
        node.remove();
        node = null;
      }
      if (notify) emit('stopped');
    }
    const live = () => {
      if (ended) throw Error('Local video stopped');
    };
    const fingerprint = (bytes) =>
      Array.from(new Uint8Array(bytes), (value) =>
        value.toString(16).padStart(2, '0').toUpperCase(),
      ).join(':');
    async function inspect() {
      live();
      if (!pc || pc.connectionState !== 'connected') return null;
      const media = provider
        ? pc.getSenders().find((sender) => sender.track?.kind === 'video')
        : pc.getReceivers().find((receiver) => receiver.track?.kind === 'video');
      const dtls = media?.transport;
      if (!dtls || dtls.state !== 'connected') return null;
      stage = 'certificates';
      const certificates = dtls.getRemoteCertificates();
      if (certificates.length !== 1) throw Error('Certificate unavailable');
      const peer = 'sha-256 ' + fingerprint(await crypto.subtle.digest('SHA-256', certificates[0]));
      live();
      stage = 'certificate-match';
      if (peer !== expectedPeer) throw Error('Certificate mismatch');
      stage = 'stats';
      const stats = await pc.getStats();
      live();
      const transports = [...stats.values()].filter(
        (record) =>
          record.type === 'transport' &&
          record.dtlsState === 'connected' &&
          record.selectedCandidatePairId,
      );
      stage = 'transport';
      if (transports.length !== 1) throw Error('Ambiguous transport');
      const pair = stats.get(transports[0].selectedCandidatePairId),
        local = stats.get(pair?.localCandidateId),
        remote = stats.get(pair?.remoteCandidateId);
      stage = 'route';
      if (
        !pair ||
        pair.state !== 'succeeded' ||
        !local ||
        !remote ||
        local.protocol !== 'udp' ||
        remote.protocol !== 'udp' ||
        !['host', 'prflx'].includes(local.candidateType) ||
        !['host', 'prflx'].includes(remote.candidateType) ||
        !Number.isInteger(local.port) ||
        !Number.isInteger(remote.port) ||
        typeof remote.address !== 'string'
      )
        throw Error('Route unavailable');
      return {
        peerFingerprint: peer,
        pair: {
          localPort: local.port,
          remotePort: remote.port,
          remoteAddress: remote.address,
          protocol: 'udp',
        },
      };
    }
    async function report() {
      if (reporting || ended || !pendingHeartbeat) return;
      reporting = true;
      const heartbeat = pendingHeartbeat;
      try {
        const value = await inspect();
        live();
        if (!value) {
          if (exposed) throw Error('Route unavailable');
          emit('alive', heartbeat);
          return;
        }
        if (exposed && JSON.stringify(value.pair) !== approvedPair) throw Error('Route changed');
        emit('connected', { ...value, ...heartbeat });
      } catch {
        emit('failed', { stage });
        stop(false);
      } finally {
        reporting = false;
        if (pendingHeartbeat === heartbeat) pendingHeartbeat = null;
        else if (pendingHeartbeat) void report();
      }
    }
    async function gather() {
      if (pc.iceGatheringState === 'complete') return;
      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pc?.removeEventListener('icegatheringstatechange', changed);
          reject(Error('Gather timeout'));
        }, 8000);
        function changed() {
          if (ended || pc.iceGatheringState === 'complete') {
            clearTimeout(timeout);
            pc.removeEventListener('icegatheringstatechange', changed);
            ended ? reject(Error('Stopped')) : resolve();
          }
        }
        pc.addEventListener('icegatheringstatechange', changed);
        changed();
      });
      live();
    }
    async function initialize() {
      const certificate = await RTCPeerConnection.generateCertificate({
        name: 'ECDSA',
        namedCurve: 'P-256',
      });
      live();
      const hashes = certificate.getFingerprints().filter((value) => value.algorithm === 'sha-256');
      if (hashes.length !== 1) throw Error('Certificate unavailable');
      ownFingerprint = 'sha-256 ' + hashes[0].value.toUpperCase();
      pc = new RTCPeerConnection({
        iceServers: [],
        certificates: [certificate],
        bundlePolicy: 'max-bundle',
        rtcpMuxPolicy: 'require',
      });
      pc.onconnectionstatechange = () => {
        if (['failed', 'disconnected', 'closed'].includes(pc.connectionState)) {
          emit('failed');
          stop(false);
        } else if (pc.connectionState === 'connected') void report();
      };
      pc.ontrack = (event) => {
        if (provider || track || event.track.kind !== 'video') {
          emit('failed');
          stop(false);
          return;
        }
        track = event.track;
        track.addEventListener('ended', () => stop(), { once: true });
      };
    }
    async function receive(message) {
      if (
        !message ||
        message.document !== documentId ||
        !/^([a-f0-9]{48})$/.test(message.operation)
      )
        throw Error('Invalid operation');
      if (operation && operation !== message.operation) throw Error('Invalid operation');
      if (message.type === 'heartbeat') {
        live();
        if (
          !Number.isInteger(message.sequence) ||
          message.sequence <= lastSequence ||
          !/^[a-f0-9]{32}$/.test(message.challenge)
        )
          return;
        lastSequence = message.sequence;
        lastHeartbeat = performance.now();
        pendingHeartbeat = { sequence: message.sequence, challenge: message.challenge };
        void report();
        return;
      }
      if (message.type === 'stop') {
        stop();
        return;
      }
      if (ended) throw Error('Stopped');
      if (message.type === 'start' && provider && !pc) {
        await initialize();
        live();
        let stream;
        if (message.source?.kind === 'synthetic') {
          const canvas = document.createElement('canvas');
          canvas.width = 320;
          canvas.height = 180;
          const ctx = canvas.getContext('2d');
          ctx.fillStyle = '#20c080';
          ctx.fillRect(0, 0, 320, 180);
          stream = canvas.captureStream(10);
          drawTimer = setInterval(() => {
            ctx.fillStyle = '#20c080';
            ctx.fillRect(0, 0, 320, 180);
          }, 100);
        } else {
          stage = 'capture';
          if (
            message.source?.kind !== 'desktop' ||
            !/^(screen|window):\d+:\d+$/.test(message.source.id)
          )
            throw Error('Invalid source');
          stream = await navigator.mediaDevices.getUserMedia({
            audio: false,
            video: {
              mandatory: {
                chromeMediaSource: 'desktop',
                chromeMediaSourceId: message.source.id,
                maxFrameRate: 30,
              },
            },
          });
        }
        if (ended) {
          stream.getTracks().forEach((item) => item.stop());
          return;
        }
        if (stream.getTracks().length !== 1 || stream.getVideoTracks().length !== 1) {
          stream.getTracks().forEach((item) => item.stop());
          throw Error('Unexpected track');
        }
        track = stream.getVideoTracks()[0];
        track.addEventListener('ended', () => stop(), { once: true });
        const sender = pc.addTrack(track, stream);
        pc.getTransceivers().find((value) => value.sender === sender).direction = 'sendonly';
        await pc.setLocalDescription(await pc.createOffer());
        await gather();
        live();
        emit('offer', { sdp: pc.localDescription.sdp, fingerprint: ownFingerprint });
        return;
      }
      if (message.type === 'offer' && !provider && !pc) {
        expectedPeer = message.fingerprint;
        await initialize();
        live();
        await pc.setRemoteDescription({ type: 'offer', sdp: message.sdp });
        live();
        for (const transceiver of pc.getTransceivers()) transceiver.direction = 'recvonly';
        await pc.setLocalDescription(await pc.createAnswer());
        await gather();
        live();
        emit('answer', { sdp: pc.localDescription.sdp, fingerprint: ownFingerprint });
        return;
      }
      if (message.type === 'answer' && provider && pc?.signalingState === 'have-local-offer') {
        stage = 'answer';
        expectedPeer = message.fingerprint;
        await pc.setRemoteDescription({ type: 'answer', sdp: message.sdp });
        live();
        void report();
        return;
      }
      if (
        message.type === 'expose' &&
        !provider &&
        !exposed &&
        pc?.connectionState === 'connected'
      ) {
        const value = await inspect();
        live();
        if (
          !value ||
          value.peerFingerprint !== message.fingerprint ||
          JSON.stringify(value.pair) !== JSON.stringify(message.pair) ||
          !track ||
          track.readyState !== 'live'
        )
          throw Error('Route changed');
        approvedPair = JSON.stringify(value.pair);
        if (message.publicOperation !== null && (typeof message.publicOperation !== 'string' || !/^[A-Za-z0-9_-]{8,128}$/.test(message.publicOperation)))
          throw Error('Invalid capture publication');
        node = document.createElement('video');
        node.id = 'palisade-capture-' + operation;
        if (message.publicOperation !== null) node.setAttribute('data-palisade-assist-capture', message.publicOperation);
        node.hidden = true;
        node.srcObject = new MediaStream([track]);
        document.body.append(node);
        exposed = true;
        emit('exposed', { elementId: node.id, publicOperation: message.publicOperation });
        return;
      }
      throw Error('Invalid transition');
    }
    return {
      operation,
      receive,
      stop,
      ended: () => ended,
      fail: () => {
        emit('failed', { stage });
        stop(false);
      },
    };
  }
  ipcRenderer.on(command, (_event, message) => {
    if (!message || message.document !== documentId || !/^[a-f0-9]{48}$/.test(message.operation))
      return;
    const starts = provider ? message.type === 'start' : message.type === 'offer';
    if (!current || current.ended()) {
      if (
        message.type === 'heartbeat' &&
        Number.isInteger(message.sequence) &&
        /^[a-f0-9]{32}$/.test(message.challenge)
      ) {
        ipcRenderer.send(channel, {
          type: 'alive',
          document: documentId,
          operation: message.operation,
          sequence: message.sequence,
          challenge: message.challenge,
        });
        return;
      }
      if (!starts || retired.has(message.operation) || retired.size >= 128) return;
      retired.add(message.operation);
      current = createPeer(message.operation);
    }
    if (current.operation !== message.operation) return;
    const session = current;
    void session.receive(message).catch(() => session.fail());
  });
  window.addEventListener('pagehide', () => current?.stop(), { once: true });
  window.addEventListener('unload', () => current?.stop(false), { once: true });
  window.addEventListener(
    'DOMContentLoaded',
    () =>
      ipcRenderer.send(channel, {
        type: 'ready',
        document: documentId,
        operation: null,
        role: provider ? 'provider' : 'consumer',
      }),
    { once: true },
  );
})();
