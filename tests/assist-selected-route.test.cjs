'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const { selectedRoute } = require('../src/assist/local-video-preload.cjs');
const pair = () => ({ local: { protocol: 'udp', type: 'host', port: 12345 }, remote: { protocol: 'udp', type: 'prflx', port: 23456, address: '127.0.0.1' } });
test('capture reads the current selected ICE route on every check without cached stats', () => {
  let current = pair(), reads = 0; const ice = { state: 'connected', getSelectedCandidatePair: () => { reads++; return current; } };
  assert.deepEqual(selectedRoute(ice), { localPort: 12345, remotePort: 23456, remoteAddress: '127.0.0.1', protocol: 'udp' });
  current = pair(); current.remote.port = 34567; assert.equal(selectedRoute(ice).remotePort, 34567); assert.equal(reads, 2);
  ice.state = 'disconnected'; assert.throws(() => selectedRoute(ice));
});
test('the exact Chromium redaction stays a marker for native verification, never an inferred IP', () => {
  const candidate=pair();candidate.remote.address='redacted-ip.invalid';
  assert.equal(selectedRoute({state:'connected',getSelectedCandidatePair:()=>candidate}).remoteAddress,'redacted-ip.invalid');
});
test('unknown, incomplete, nonlocal-format or unsupported selected routes fail closed', () => {
  for (const candidate of [null, {}, { ...pair(), local: null }, { ...pair(), remote: null }]) assert.throws(() => selectedRoute({ state: 'connected', getSelectedCandidatePair: () => candidate }));
  for (const side of ['local', 'remote']) for (const change of [{ protocol: 'tcp' }, { type: 'relay' }, { type: 'srflx' }, { port: null }, { port: 0 }, { port: 65536 }, { port: 1.5 }]) {
    const candidate = pair(); Object.assign(candidate[side], change); assert.throws(() => selectedRoute({ state: 'completed', getSelectedCandidatePair: () => candidate }));
  }
  for (const address of [undefined, 'name.local', '::1', '127.0.0.999','redacted-literal.invalid','unknown','redacted-ip.invalid.evil']) { const candidate = pair(); candidate.remote.address = address; assert.throws(() => selectedRoute({ state: 'connected', getSelectedCandidatePair: () => candidate })); }
  assert.throws(() => selectedRoute({ state: 'connected' }));
});
