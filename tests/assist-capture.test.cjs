'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { createCaptureController } = require('../src/assist/capture.cjs');
function fixture(options = {}) {
  const frame = { url: 'https://palisade.onyxops.org/' }, sources = [{ id: 'screen:0:0', name: 'Display 1' }, { id: 'window:11:0', name: 'Fixture' }];
  let calls = 0, revoked = 0;
  const contents = { mainFrame: frame, getURL: () => frame.url, isDestroyed: () => false };
  const owner = { isDestroyed: () => false, isVisible: () => true, isMinimized: () => false };
  const controller = createCaptureController({ enabled: true, getContents: () => contents, getWindow: () => owner,
    enumerate: async () => { calls++; return sources; }, choose: async () => null, revoke: () => revoked++, ...options });
  const request = { frame, securityOrigin: frame.url, videoRequested: true, audioRequested: false, userGesture: true };
  return { controller, request, contents, owner, sources, calls: () => calls, revoked: () => revoked };
}
test('capture denies wrong frame/origin/no gesture/audio/tray requests before enumeration', async () => {
  const h = fixture();
  for (const change of [{ frame: {} }, { securityOrigin: 'https://evil.test' }, { userGesture: false }, { audioRequested: true }, { videoRequested: false }]) {
    await h.controller.handle({ ...h.request, ...change }, value => assert.equal(value, null));
  }
  h.owner.isVisible = () => false;
  await h.controller.handle(h.request, value => assert.equal(value, null));
  h.owner.isVisible = () => true;
  Object.defineProperty(h.contents, 'mainFrame', { get() { throw new Error('Detached frame'); } });
  await h.controller.handle(h.request, value => assert.equal(value, null));
  assert.equal(h.calls(), 0); h.controller.dispose();
});
test('source cancellation, unknown selection and disappeared sources grant nothing', async () => {
  for (const selection of [null, 'screen:999:0']) {
    const h = fixture({ choose: async () => selection });
    await h.controller.handle(h.request, value => assert.equal(value, null)); h.controller.dispose();
  }
  let calls = 0;
  const h = fixture({ choose: async () => 'window:11:0', enumerate: async () => ++calls === 1 ? [{ id: 'window:11:0', name: 'Fixture' }] : [] });
  await h.controller.handle(h.request, value => assert.equal(value, null)); h.controller.dispose();
});
test('explicit selection grants only the selected video source; stop revokes and late choices cannot restart', async () => {
  const h = fixture({ choose: async () => 'window:11:0' }); let granted;
  await h.controller.handle(h.request, value => { granted = value; });
  assert.equal(granted.video.id, 'window:11:0'); assert.equal(Object.keys(granted).join(','), 'video');
  h.controller.stop(); assert.equal(h.revoked(), 1); assert.equal(h.controller.busy(), false);
  let select; const late = fixture({ choose: () => new Promise(resolve => { select = resolve; }) });
  const pending = late.controller.handle(late.request, value => assert.equal(value, null));
  await new Promise(resolve => setImmediate(resolve)); late.controller.stop(); select('screen:0:0'); await pending;
  assert.equal(late.controller.busy(), false); late.controller.dispose(); h.controller.dispose();
});
test('picker timeout settles once and disabled production policy cannot enumerate', async () => {
  const h = fixture({ timeoutMs: 10, choose: () => new Promise(resolve => { setTimeout(() => resolve('screen:0:0'), 30); }) });
  let callbacks = 0;
  await h.controller.handle(h.request, value => { callbacks++; assert.equal(value, null); });
  assert.equal(callbacks, 1); h.controller.dispose();
  const disabled = fixture({ enabled: false });
  await disabled.controller.handle(disabled.request, value => assert.equal(value, null)); assert.equal(disabled.calls(), 0);
  assert.equal(require('../src/assist/build-policy.cjs').captureEnabled, false);
});
