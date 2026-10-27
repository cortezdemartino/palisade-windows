'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const { dataSdp, candidate, inputPacket } = require('../src/assist/remote-input-policy.cjs');
const fp = 'sha-256 ' + Array(32).fill('AB').join(':');
const sdp = () => ['v=0', 'o=- 123 2 IN IP4 127.0.0.1', 's=-', 't=0 0', 'a=group:BUNDLE 0',
  'a=msid-semantic: WMS', 'm=application 9 UDP/DTLS/SCTP webrtc-datachannel', 'c=IN IP4 0.0.0.0',
  'a=ice-ufrag:abcd', 'a=ice-pwd:abcdefghijklmnopqrstuvwxyz', `a=fingerprint:${fp}`,
  'a=setup:actpass', 'a=mid:0', 'a=sctp-port:5000', 'a=max-message-size:262144'].join('\r\n') + '\r\n';
test('remote input SDP binds one data-only transport to the approved certificate', () => {
  assert.equal(dataSdp(sdp(), 'offer', fp).mid, '0');
  assert.equal(dataSdp(sdp().replace('actpass', 'active'), 'answer', fp).fingerprint, fp);
  assert.throws(() => dataSdp(sdp(), 'offer', fp.replaceAll('AB', 'CD')));
  for (const extra of [`a=fingerprint:${fp}`, 'm=audio 9 UDP/TLS/RTP/SAVPF 0', 'a=crypto:1 bad', 'a=setup:active', 'a=sctp-port:6000', 'a=ice-pwd:other'])
    assert.throws(() => dataSdp(sdp() + extra + '\r\n', 'offer', fp));
  for (const bad of [sdp().replaceAll('\r\n', '\n'), sdp().replace('UDP/DTLS/SCTP', 'TCP/DTLS/SCTP'), sdp().replace('BUNDLE 0', 'BUNDLE 1'), sdp().replace('actpass', 'active'), 'x'.repeat(16385)])
    assert.throws(() => dataSdp(bad, 'offer', fp));
});
test('remote candidates are bounded structured ICE, never ICE-server configuration', () => {
  for (const value of ['', 'candidate:1 1 udp 123 127.0.0.1 5000 typ host', 'candidate:2 1 udp 123 ::1 5000 typ host', 'candidate:3 1 udp 123 fixture.local 5000 typ srflx raddr 0.0.0.0 rport 0', 'candidate:4 1 tcp 123 127.0.0.1 9 typ host tcptype active']) candidate(value, '0', '0');
  for (const value of ['candidate:1 1 udp 123 https://example.test 5000 typ host', 'candidate:1 1 udp 123 127.0.0.1 65536 typ host', 'candidate:1 1 udp 123 127.0.0.1 5000 typ host\r\na=evil', 'candidate:1 1 udp 123 127.0.0.1 5000 typ host tcptype active', 'candidate:1 1 udp 123 127.0.0.1 5000 typ host unknown value', 'x'.repeat(1025)]) assert.throws(() => candidate(value, '0', '0'));
  assert.throws(() => candidate('', '1', '0'));
});
test('remote packets retain original bytes for native strict decoding and exact session binding', () => {
  const text = JSON.stringify({ p: 'palisade.assist.control', v: 2, session: 'session_0001', kind: 'release-all' });
  assert.equal(inputPacket(text, 'session_0001'), text);
  for (const session of [undefined, 12345678, { toString: () => 'session_0001' }]) assert.throws(() => inputPacket(text, session));
  for (const bad of [null, Buffer.from(text), '{}', text.replace('session_0001', 'session_0002'), text.replace('release-all', 'file-offer'), text + 'x'.repeat(2048), '💬'.repeat(600)]) assert.throws(() => inputPacket(bad, 'session_0001'));
});
