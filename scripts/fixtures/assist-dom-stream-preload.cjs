'use strict';
// Synthetic canvas only. No desktop/media permission, network or page bridge.
const { ipcRenderer } = require('electron');
window.addEventListener(
  'DOMContentLoaded',
  () => {
    const canvas = document.createElement('canvas');
    canvas.width = 96;
    canvas.height = 64;
    const ctx = canvas.getContext('2d');
    ctx.fillStyle = '#20c080';
    ctx.fillRect(0, 0, 96, 64);
    const stream = canvas.captureStream(1),
      video = document.createElement('video');
    video.id = 'owned-synthetic-stream';
    video.hidden = true;
    video.srcObject = stream;
    document.body.append(video);
    ipcRenderer.send('palisade:test-stream-ready');
    window.addEventListener('unload', () => stream.getTracks().forEach((track) => track.stop()), {
      once: true,
    });
  },
  { once: true },
);
