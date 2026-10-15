'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const {
  PALISADE_URL,
  isPalisadeUrl,
  normaliseExternalUrl,
  mayGrantPermission,
} = require('../src/security-policy.cjs');

test('only exact Palisade HTTPS origin may host the renderer', () => {
  for (const url of [PALISADE_URL, 'https://palisade.onyxops.org/join#code', 'https://PALISADE.ONYXOPS.ORG/register']) {
    assert.equal(isPalisadeUrl(url), true, url);
  }
  for (const url of [
    'http://palisade.onyxops.org/',
    'http://localhost:5173/',
    'https://palisade.onyxops.org.evil.test/',
    'https://sub.palisade.onyxops.org/',
    'https://palisade.onyxops.org@evil.test/',
    'https://admin:p@palisade.onyxops.org/',
    'file:///C:/Windows/system.ini',
    'javascript:alert(1)',
    'data:text/html,evil',
    'not a URL',
  ]) assert.equal(isPalisadeUrl(url), false, url);
});

test('external links accept only credential-free HTTPS and are never Palisade itself', () => {
  assert.equal(normaliseExternalUrl('https://example.com/a?b=c'), 'https://example.com/a?b=c');
  for (const url of [
    'https://palisade.onyxops.org/channels', 'http://example.com', 'javascript:alert(1)',
    'file:///C:/foo', 'mailto:test@example.com', 'https://x:y@example.com/',
    'https://example.com/\nattack', 'x'.repeat(2200),
  ]) assert.equal(normaliseExternalUrl(url), null, url.slice(0, 90));
});

test('only microphone audio and notifications are approved for Palisade origin', () => {
  assert.equal(mayGrantPermission('media', PALISADE_URL, ['audio']), true);
  assert.equal(mayGrantPermission('notifications', PALISADE_URL), true);
  assert.equal(mayGrantPermission('media', PALISADE_URL, ['video']), false);
  assert.equal(mayGrantPermission('media', PALISADE_URL, ['audio', 'video']), false);
  assert.equal(mayGrantPermission('media', PALISADE_URL), false);
  for (const permission of ['display-capture', 'camera', 'geolocation', 'clipboard-read', 'openExternal', 'midi', 'usb', 'serial', 'mediaKeySystem']) {
    assert.equal(mayGrantPermission(permission, PALISADE_URL, ['audio']), false, permission);
  }
  assert.equal(mayGrantPermission('media', 'https://challenges.cloudflare.com/', ['audio']), false);
});
