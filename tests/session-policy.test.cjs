'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { installSessionPolicy } = require('../src/session-policy.cjs');
const { PALISADE_URL } = require('../src/security-policy.cjs');

function fixture() {
  let current;
  const profile = {
    webRequest: { onBeforeRequest(_filter, handler) { profile.navigation = handler; } },
    setPermissionRequestHandler(handler) { this.request = handler; },
    setPermissionCheckHandler(handler) { this.check = handler; },
    setDevicePermissionHandler(handler) { this.device = handler; },
    setDisplayMediaRequestHandler(handler, options) { this.display = handler; this.options = options; },
  };
  const contents = { isDestroyed: () => false, getURL: () => PALISADE_URL };
  current = contents;
  installSessionPolicy(profile, () => current);
  const details = { isMainFrame: true, requestingUrl: PALISADE_URL, mediaType: 'audio', mediaTypes: ['audio'] };
  return { profile, contents, details, replace: value => { current = value; },
    request(requester, permission, value) {
      let count = 0, result;
      profile.request(requester, permission, grant => { count++; result = grant; }, value);
      assert.equal(count, 1); return result;
    },
  };
}

test('permission checks and requests preserve trusted microphone and notification access only', () => {
  const h = fixture();
  for (const permission of ['media', 'notifications']) {
    assert.equal(h.request(h.contents, permission, h.details), true);
    assert.equal(h.profile.check(h.contents, permission, PALISADE_URL, h.details), true);
  }
  for (const mediaType of ['video', 'unknown', undefined]) {
    assert.equal(h.profile.check(h.contents, 'media', PALISADE_URL, { ...h.details, mediaType }), false);
  }
  for (const mediaTypes of [['video'], ['audio', 'video'], [], undefined]) {
    assert.equal(h.request(h.contents, 'media', { ...h.details, mediaTypes }), false);
  }
  for (const permission of ['display-capture', 'camera', 'clipboard-read', 'clipboard-sanitized-write',
    'fileSystem', 'keyboardLock', 'pointerLock', 'captured-surface-control', 'window-management', 'hid', 'usb', 'serial', 'unknown']) {
    assert.equal(h.request(h.contents, permission, h.details), false, permission);
    assert.equal(h.profile.check(h.contents, permission, PALISADE_URL, h.details), false, permission);
  }
  assert.equal(h.profile.device({ origin: PALISADE_URL }), false);
});

test('same-origin subframes, spoofed contents and missing or contradictory frame provenance fail closed', () => {
  const h = fixture();
  for (const requester of [null, {}, { ...h.contents }]) {
    assert.equal(h.request(requester, 'media', h.details), false);
    assert.equal(h.profile.check(requester, 'media', PALISADE_URL, h.details), false);
  }
  for (const details of [undefined, {}, { ...h.details, isMainFrame: false },
    { ...h.details, isMainFrame: 'true' }, { ...h.details, requestingUrl: 'https://evil.test/' },
    { ...h.details, securityOrigin: 'https://evil.test/' }, { ...h.details, embeddingOrigin: 'https://evil.test/' }]) {
    assert.equal(h.request(h.contents, 'media', details), false);
    assert.equal(h.profile.check(h.contents, 'media', PALISADE_URL, details), false);
  }
  assert.equal(h.request(h.contents, 'media', { ...h.details, requestingUrl: undefined }), false);
  for (const origin of ['', undefined, 'https://palisade.onyxops.org.evil.test', 'http://palisade.onyxops.org']) {
    assert.equal(h.profile.check(h.contents, 'media', origin, h.details), false);
  }
});

test('navigation, destruction and contents replacement invalidate permission authority immediately', () => {
  const h = fixture();
  h.contents.getURL = () => 'https://evil.test/';
  assert.equal(h.request(h.contents, 'media', h.details), false);
  h.contents.getURL = () => PALISADE_URL;
  h.contents.isDestroyed = () => true;
  assert.equal(h.profile.check(h.contents, 'media', PALISADE_URL, h.details), false);
  h.contents.isDestroyed = () => false;
  h.replace({ ...h.contents });
  assert.equal(h.request(h.contents, 'media', h.details), false);
  h.replace(null);
  assert.equal(h.request(h.contents, 'notifications', h.details), false);
  let blocked;
  h.profile.navigation({ resourceType: 'mainFrame', url: 'https://evil.test/' }, value => { blocked = value.cancel; });
  assert.equal(blocked, true);
  h.profile.navigation({ resourceType: 'mainFrame', url: PALISADE_URL }, value => { blocked = value.cancel; });
  assert.equal(blocked, false);
});

test('display requests never select or enumerate sources while capture is disabled', () => {
  const h = fixture();
  assert.equal(h.profile.options.useSystemPicker, false);
  for (const request of [null, {}, { frame: h.contents, securityOrigin: PALISADE_URL, userGesture: true,
    videoRequested: true, audioRequested: false }, { securityOrigin: 'https://evil.test/', audioRequested: true }]) {
    let count = 0;
    h.profile.display(request, streams => { count++; assert.equal(streams, null); });
    assert.equal(count, 1);
  }
});
test('an available controller cannot turn ambiguous empty media requests into capture permission', () => {
  const h = fixture();
  installSessionPolicy(h.profile, () => h.contents, { available: () => true, handle: (_request, done) => done(null) });
  assert.equal(h.request(h.contents, 'media', { ...h.details, mediaTypes: [] }), false);
  assert.equal(h.profile.check(h.contents, 'media', PALISADE_URL, { ...h.details, mediaType: 'unknown' }), false);
});
