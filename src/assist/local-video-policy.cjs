'use strict';
const { isIP } = require('node:net');
const FP = /^sha-256 (?:[0-9A-F]{2}:){31}[0-9A-F]{2}$/;
const failure = () => {
  throw new Error('Local video negotiation refused');
};
const ip4 = (value) =>
  typeof value === 'string' &&
  isIP(value) === 4 &&
  !['0.0.0.0', '255.255.255.255'].includes(value) &&
  Number(value.split('.')[0]) < 224;
function parse(sdp, direction, setup) {
  if (
    typeof sdp !== 'string' ||
    Buffer.byteLength(sdp) > 65536 ||
    !sdp.endsWith('\r\n') ||
    /[^\x20-\x7e\r\n]/.test(sdp) ||
    /(?<!\r)\n|\r(?!\n)/.test(sdp)
  )
    failure();
  const lines = sdp.slice(0, -2).split('\r\n');
  if (lines.length > 256 || lines.some((line) => !line || line.length > 4096)) failure();
  const count = (prefix) => lines.filter((line) => line.startsWith(prefix));
  if (
    count('m=').length !== 1 ||
    !/^m=video \d{1,5} UDP\/TLS\/RTP\/SAVPF(?: \d{1,3})+$/.test(count('m=')[0]) ||
    count('c=').length !== 1 ||
    !/^c=IN IP4 [0-9.]+$/.test(count('c=')[0])
  )
    failure();
  const mediaPort = Number(count('m=')[0].split(' ')[1]),
    connection = count('c=')[0].slice(9);
  if (
    mediaPort < 1 ||
    mediaPort > 65535 ||
    (connection !== '0.0.0.0' && !ip4(connection)) ||
    (mediaPort === 9 && connection !== '0.0.0.0')
  )
    failure();
  for (const prefix of ['a=fingerprint:', 'a=ice-ufrag:', 'a=ice-pwd:', 'a=mid:', 'a=setup:'])
    if (count(prefix).length !== 1) failure();
  const fingerprint = count('a=fingerprint:')[0].slice(14);
  if (
    !FP.test(fingerprint) ||
    count('a=mid:')[0] !== 'a=mid:0' ||
    count('a=setup:')[0] !== setup ||
    !lines.includes(direction) ||
    lines.filter((line) => /^a=(sendrecv|sendonly|recvonly|inactive)$/.test(line)).length !== 1 ||
    !lines.includes('a=rtcp-mux')
  )
    failure();
  if (
    !/^a=ice-ufrag:[A-Za-z0-9+/]{4,256}$/.test(count('a=ice-ufrag:')[0]) ||
    !/^a=ice-pwd:[A-Za-z0-9+/]{22,256}$/.test(count('a=ice-pwd:')[0])
  )
    failure();
  const allowed =
    /^(?:v=0|o=- \d+ \d+ IN IP4 127\.0\.0\.1|s=-|t=0 0|m=video .+|c=IN IP4 [0-9.]+|a=group:BUNDLE 0|a=extmap-allow-mixed|a=msid-semantic: WMS(?: [A-Za-z0-9_-]+)*|a=rtcp:9 IN IP4 0\.0\.0\.0|a=ice-ufrag:.+|a=ice-pwd:.+|a=ice-options:trickle|a=fingerprint:.+|a=setup:.+|a=mid:0|a=(?:sendonly|recvonly)|a=msid:[A-Za-z0-9_-]+ [A-Za-z0-9_-]+|a=rtcp-mux|a=rtcp-rsize|a=rtcp-xr:rcvr-rtt=all|a=(?:extmap|rtpmap|rtcp-fb|fmtp|ssrc|ssrc-group):.+|a=end-of-candidates|a=candidate:.+)$/;
  if (lines.some((line) => !allowed.test(line))) failure();
  const candidates = [];
  for (const line of count('a=candidate:')) {
    const words = line.slice(12).split(' ');
    if (
      words.length < 8 ||
      words.length > 22 ||
      !/^[A-Za-z0-9+/]{1,32}$/.test(words[0]) ||
      !/^\d+$/.test(words[1]) ||
      !['udp', 'tcp'].includes(words[2]) ||
      !/^\d{1,10}$/.test(words[3]) ||
      Number(words[3]) > 0xffffffff ||
      !/^\d{1,5}$/.test(words[5]) ||
      Number(words[5]) < 1 ||
      Number(words[5]) > 65535 ||
      words[6] !== 'typ' ||
      !['host', 'srflx', 'relay', 'prflx'].includes(words[7])
    )
      failure();
    const address = words[4],
      extensions = words.slice(8);
    if (extensions.length % 2) failure();
    const names = new Set();
    for (let i = 0; i < extensions.length; i += 2) {
      const key = extensions[i],
        value = extensions[i + 1];
      if (names.has(key)) failure();
      names.add(key);
      if (
        !{
          generation: /^0$/,
          ufrag: /^[A-Za-z0-9+/]{4,256}$/,
          'network-id': /^\d{1,8}$/,
          'network-cost': /^\d{1,4}$/,
          tcptype: /^(active|passive|so)$/,
          raddr: /^[A-Za-z0-9.:-]+$/,
          rport: /^\d{1,5}$/,
        }[key]?.test(value)
      )
        failure();
    }
    if (!(isIP(address) || /^[A-Za-z0-9-]{1,63}\.local$/.test(address))) failure();
    // Unsupported routes are discarded, never forwarded to either owned peer.
    if (words[1] !== '1' || words[2] !== 'udp' || words[7] !== 'host' || isIP(address) === 6)
      continue;
    if (names.has('raddr') || names.has('rport') || names.has('tcptype')) failure();
    candidates.push({ foundation: words[0], priority: words[3], port: Number(words[5]) });
  }
  if (
    !candidates.length ||
    candidates.length > 64 ||
    new Set(candidates.map((c) => c.port)).size !== candidates.length
  )
    failure();
  return { lines, candidates, fingerprint, mediaPort, connection };
}
async function sanitize(sdp, { direction, setup, inspect, signal }) {
  const result = parse(sdp, direction, setup);
  if (signal?.aborted) failure();
  const response = await inspect(result.candidates.map((c) => c.port));
  if (signal?.aborted) failure();
  if (
    !response ||
    Object.keys(response).join(',') !== 'candidates' ||
    !Array.isArray(response.candidates) ||
    response.candidates.length !== result.candidates.length
  )
    failure();
  const bindings = new Map();
  for (const row of response.candidates) {
    if (
      Object.keys(row).sort().join(',') !== 'address,port' ||
      !ip4(row.address) ||
      !result.candidates.some((c) => c.port === row.port) ||
      bindings.has(row.port)
    )
      failure();
    bindings.set(row.port, row.address);
  }
  if (result.mediaPort !== 9 && bindings.get(result.mediaPort) !== result.connection) failure();
  const lines = result.lines
    .filter((line) => !line.startsWith('a=candidate:') && line !== 'a=end-of-candidates')
    .map((line) =>
      line.startsWith('m=video ')
        ? line.replace(/^m=video \d+ /, 'm=video 9 ')
        : line.startsWith('c=')
          ? 'c=IN IP4 0.0.0.0'
          : line,
    );
  for (const candidate of result.candidates)
    lines.push(
      `a=candidate:${candidate.foundation} 1 udp ${candidate.priority} ${bindings.get(candidate.port)} ${candidate.port} typ host`,
    );
  lines.push('a=end-of-candidates');
  return { sdp: lines.join('\r\n') + '\r\n', fingerprint: result.fingerprint, bindings };
}
function pairAllowed(pair, local, remote) {
  return Boolean(
    pair &&
      Object.keys(pair).sort().join(',') === 'localPort,protocol,remoteAddress,remotePort' &&
      pair.protocol === 'udp' &&
      local.has(pair.localPort) &&
      remote.get(pair.remotePort) === pair.remoteAddress,
  );
}
module.exports = { parse, sanitize, pairAllowed, FP };
