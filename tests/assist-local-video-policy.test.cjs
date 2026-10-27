'use strict';
const test = require('node:test'),
  assert = require('node:assert/strict');
const { parse, sanitize, pairAllowed } = require('../src/assist/local-video-policy.cjs');
const fp = 'sha-256 ' + Array(32).fill('AB').join(':');
const offer = (
  candidates = [
    'a=candidate:1 1 udp 2113937151 550e8400-e29b-41d4-a716-446655440000.local 54001 typ host generation 0 network-cost 999',
  ],
) =>
  [
    'v=0',
    'o=- 123 2 IN IP4 127.0.0.1',
    's=-',
    't=0 0',
    'a=group:BUNDLE 0',
    'm=video 9 UDP/TLS/RTP/SAVPF 96',
    'c=IN IP4 0.0.0.0',
    'a=ice-ufrag:abcd',
    'a=ice-pwd:abcdefghijklmnopqrstuvwxyz',
    `a=fingerprint:${fp}`,
    'a=setup:actpass',
    'a=mid:0',
    'a=sendonly',
    'a=rtcp-mux',
    'a=rtpmap:96 VP8/90000',
    ...candidates,
  ].join('\r\n') + '\r\n';
test('local video replaces mDNS only with independently owned addresses and omits unsupported routes', async () => {
  const sdp = offer([
    'a=candidate:1 1 udp 2 peer.local 54001 typ host',
    'a=candidate:2 1 tcp 2 peer.local 9 typ host tcptype passive',
    'a=candidate:3 1 udp 2 2001:db8::1 54002 typ host',
  ]);
  const value = await sanitize(sdp, {
    direction: 'a=sendonly',
    setup: 'a=setup:actpass',
    inspect: async (ports) => {
      assert.deepEqual(ports, [54001]);
      return { candidates: [{ port: 54001, address: '127.0.0.1' }] };
    },
  });
  assert.equal(value.fingerprint, fp);
  assert(value.sdp.includes('127.0.0.1 54001 typ host'));
  assert(!value.sdp.includes('.local'));
  assert(!value.sdp.includes(' tcp '));
  assert(!value.sdp.includes('2001:'));
});
test('alternate media, fingerprints, credentials, directions and embedded routes fail closed', () => {
  for (const sdp of [
    offer() + 'm=audio 9 UDP/TLS/RTP/SAVPF 111\r\n',
    offer().replace('m=video', 'm=application'),
    offer() + `a=fingerprint:${fp}\r\n`,
    offer() + 'a=ice-ufrag:other\r\n',
    offer().replace('a=sendonly', 'a=sendrecv'),
    offer() + 'a=remote-candidates:1 203.0.113.1 22\r\n',
    offer().replace('c=IN IP4 0.0.0.0', 'c=IN IP4 203.0.113.1'),
    offer().replaceAll('\r\n', '\n'),
    offer(['a=candidate:1 1 udp 2 evil.local.attacker.test 54001 typ host']),
  ])
    assert.throws(() => parse(sdp, 'a=sendonly', 'a=setup:actpass'));
});
test('unowned, ambiguous and stale ownership never creates a remote description', async () => {
  for (const inspect of [
    async () => ({ error: 'ice-port-unowned' }),
    async () => ({ candidates: [] }),
    async () => ({ candidates: [{ port: 54001, address: '0.0.0.0' }] }),
    async () => ({ candidates: [{ port: 54002, address: '127.0.0.1' }] }),
  ])
    await assert.rejects(
      sanitize(offer(), { direction: 'a=sendonly', setup: 'a=setup:actpass', inspect }),
    );
  const abort = new AbortController();
  await assert.rejects(
    sanitize(offer(), {
      direction: 'a=sendonly',
      setup: 'a=setup:actpass',
      signal: abort.signal,
      inspect: async () => {
        abort.abort();
        return { candidates: [{ port: 54001, address: '127.0.0.1' }] };
      },
    }),
  );
});
test('selected pair must retain exact locally proven UDP ports and destination', () => {
  const local = new Map([[54001, '127.0.0.1']]),
    remote = new Map([[54002, '127.0.0.1']]);
  const pair = { localPort: 54001, remotePort: 54002, remoteAddress: '127.0.0.1', protocol: 'udp' };
  assert(pairAllowed(pair, local, remote));
  for (const change of [
    { remoteAddress: '203.0.113.1' },
    { remotePort: 54003 },
    { localPort: 54003 },
    { protocol: 'tcp' },
    { extra: true },
  ])
    assert.equal(pairAllowed({ ...pair, ...change }, local, remote), false);
});
test('liveness challenges cannot be replayed or renewed by delayed queued reports', () => {
  const { Liveness } = require('../src/assist/local-video-liveness.cjs');
  let now = 100;
  const clock = new Liveness(() => now);
  const issued = clock.issue('consumer');
  assert.equal(clock.issue('consumer'), null);
  const reply = { sequence: issued.sequence, challenge: issued.challenge };
  now = 300;
  assert.equal(clock.match('consumer', { ...reply, challenge: '0'.repeat(32) }, 200), null);
  assert.equal(clock.match('consumer', reply, 200), issued);
  now = 1600;
  assert.equal(clock.match('consumer', reply, 200), null);
  assert.throws(() => clock.finish('consumer', issued, true));
  assert(clock.expired('consumer'));
  now = 100;
  const other = new Liveness(() => now),
    fresh = other.issue('provider');
  now = 200;
  other.finish('provider', fresh, true);
  assert.equal(
    other.match('provider', { sequence: fresh.sequence, challenge: fresh.challenge }, 150),
    null,
  );
  now = 1599;
  assert.equal(other.expired('provider'), false);
  now = 1600;
  assert.equal(other.expired('provider'), true);
});
