'use strict';
// Internal transport validation only. Grants, consent, leases and input decoding
// remain native-runtime responsibilities; SDP never supplies authorization.
const { isIP } = require('node:net');
const FP = /^sha-256 (?:[0-9A-F]{2}:){31}[0-9A-F]{2}$/;
const MID = /^[A-Za-z0-9_-]{1,32}$/;
const TOKEN = /^[A-Za-z0-9_-]{8,128}$/;
const refuse = () => { throw Error('Invalid input transport'); };
const address = value => typeof value === 'string' &&
  (isIP(value) !== 0 || /^[A-Za-z0-9-]{1,63}\.local$/.test(value));
function candidate(value, mid, expectedMid) {
  if (typeof value !== 'string' || value.length > 1024 || /[^\x20-\x7e]/.test(value) ||
      (mid !== null && (mid !== expectedMid || !MID.test(mid)))) refuse();
  if (value === '') return;
  const parts = value.split(' ');
  if (parts.length < 8 || parts.length > 24 || parts.length % 2 ||
      !/^candidate:[A-Za-z0-9+/]{1,32}$/.test(parts[0]) || parts[1] !== '1' ||
      !['udp', 'tcp'].includes(parts[2]) || !/^\d{1,10}$/.test(parts[3]) ||
      Number(parts[3]) > 0xffffffff || !address(parts[4]) || !/^\d{1,5}$/.test(parts[5]) ||
      Number(parts[5]) < 1 || Number(parts[5]) > 65535 || parts[6] !== 'typ' ||
      !['host', 'srflx', 'prflx', 'relay'].includes(parts[7])) refuse();
  const seen = new Set();
  for (let i = 8; i < parts.length; i += 2) {
    const key = parts[i], value = parts[i + 1];
    if (seen.has(key)) refuse();
    seen.add(key);
    if (key === 'raddr' ? !address(value) :
        key === 'rport' ? !/^\d{1,5}$/.test(value) || Number(value) > 65535 :
        !({ generation: /^\d{1,8}$/, ufrag: /^[A-Za-z0-9+/]{4,256}$/,
          'network-id': /^\d{1,8}$/, 'network-cost': /^\d{1,4}$/, tcptype: /^(active|passive|so)$/ }[key]?.test(value))) refuse();
  }
  if ((parts[2] === 'tcp') !== seen.has('tcptype')) refuse();
}
function dataSdp(sdp, kind, expectedFingerprint) {
  if (!['offer', 'answer'].includes(kind) || !FP.test(expectedFingerprint) ||
      typeof sdp !== 'string' || sdp.length > 16384 || !sdp.endsWith('\r\n') ||
      /[^\x20-\x7e\r\n]/.test(sdp) || /(?<!\r)\n|\r(?!\n)/.test(sdp)) refuse();
  const lines = sdp.slice(0, -2).split('\r\n');
  if (lines.length > 128 || lines.some(line => !line || line.length > 2048)) refuse();
  const single = prefix => {
    const rows = lines.filter(line => line.startsWith(prefix));
    if (rows.length !== 1) refuse();
    return rows[0].slice(prefix.length);
  };
  const media = single('m=');
  if (!/^application (?:[1-9]\d{0,4}) UDP\/DTLS\/SCTP webrtc-datachannel$/.test(media) ||
      Number(media.split(' ')[1]) > 65535) refuse();
  const connection = single('c=').split(' ');
  if (connection.length !== 3 || connection[0] !== 'IN' ||
      !['IP4', 'IP6'].includes(connection[1]) ||
      isIP(connection[2]) !== (connection[1] === 'IP4' ? 4 : 6)) refuse();
  const fingerprint = single('a=fingerprint:'), mid = single('a=mid:'), setup = single('a=setup:');
  if (fingerprint !== expectedFingerprint || !MID.test(mid) ||
      single('a=group:') !== 'BUNDLE ' + mid ||
      !(kind === 'offer' ? setup === 'actpass' : ['active', 'passive'].includes(setup)) ||
      !/^[A-Za-z0-9+/]{4,256}$/.test(single('a=ice-ufrag:')) ||
      !/^[A-Za-z0-9+/]{22,256}$/.test(single('a=ice-pwd:')) ||
      !/^\d{1,5}$/.test(single('a=sctp-port:')) ||
      Number(single('a=sctp-port:')) < 1 || Number(single('a=sctp-port:')) > 65535) refuse();
  const allowed = /^(?:v=0|o=- \d+ \d+ IN IP[46] [0-9a-fA-F:.]+|s=-|t=0 0|a=group:BUNDLE [A-Za-z0-9_-]+|a=extmap-allow-mixed|a=msid-semantic: WMS|m=application .+|c=IN IP[46] [0-9a-fA-F:.]+|a=ice-ufrag:.+|a=ice-pwd:.+|a=ice-options:trickle|a=fingerprint:.+|a=setup:.+|a=mid:.+|a=sctp-port:.+|a=max-message-size:\d{1,10}|a=candidate:.+|a=end-of-candidates)$/;
  if (lines.some(line => !allowed.test(line))) refuse();
  for (const prefix of ['v=', 'o=', 's=', 't=']) single(prefix);
  for (const prefix of ['a=ice-options:', 'a=max-message-size:', 'a=end-of-candidates', 'a=extmap-allow-mixed', 'a=msid-semantic:'])
    if (lines.filter(line => line.startsWith(prefix)).length > 1) refuse();
  const candidates = lines.filter(line => line.startsWith('a=candidate:')).map(line => line.slice(2));
  if (candidates.length > 64 || new Set(candidates).size !== candidates.length) refuse();
  for (const value of candidates) candidate(value, mid, mid);
  return Object.freeze({ fingerprint, mid, candidates: Object.freeze(candidates) });
}
function inputPacket(text, session) {
  if (typeof session !== 'string' || !TOKEN.test(session) || typeof text !== 'string' || Buffer.byteLength(text, 'utf8') > 2048 || !text) refuse();
  let value;
  try { value = JSON.parse(text); } catch { refuse(); }
  if (!value || value.p !== 'palisade.assist.control' || value.v !== 2 || value.session !== session ||
      !['pointer-move', 'button', 'wheel', 'key', 'shortcut', 'release-all'].includes(value.kind)) refuse();
  // Forward the original bytes. The native strict decoder rejects duplicates,
  // unknown fields, unapproved grants/generations, replay and invalid actions.
  return text;
}
module.exports = { FP, TOKEN, dataSdp, candidate, inputPacket };
