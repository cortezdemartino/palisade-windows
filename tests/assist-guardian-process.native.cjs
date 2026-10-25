'use strict';
// Explicit Windows executable tests. Not a desktop input/installation acceptance test.
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { randomBytes, generateKeyPairSync, sign } = require('node:crypto');
const { once } = require('node:events');
const { openGuardian } = require('../src/assist/guardian.cjs');
const binary = process.env.PALISADE_ASSIST_TEST_BINARY || path.resolve('.assist/Palisade-Assist.exe');
const manifestFile = process.env.PALISADE_ASSIST_TEST_MANIFEST || path.resolve('src/assist/guardian-integrity.json');
const hello = { type: 'hello', protocol: 'palisade.assist.guardian', version: 1, challenge: randomBytes(32).toString('base64url') };
function frame(command, sequence = 0) {
  const bytes = Buffer.from(typeof command === 'string' ? command : JSON.stringify({ sequence, command }));
  const prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length); return Buffer.concat([prefix, bytes]);
}
async function terminated(wire, { pid = process.pid, end = false, idle = false, drain = true, mode = '--probe' } = {}) {
  const started = Date.now();
  const child = spawn(binary, [mode, String(pid)], { windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] });
  child.stdin.on('error', () => {}); if (drain) child.stdout.resume();
  const exit = once(child, 'exit');
  const timeout = setTimeout(() => child.kill(), idle ? 8000 : 4000);
  try {
    if (wire) child.stdin.write(wire);
    if (end) child.stdin.end();
    const [code, signal] = await exit;
    assert.equal(signal, null, 'helper exceeded its termination deadline');
    assert.equal(code, 125);
    if (!drain) {
      const chunks = []; for await (const chunk of child.stdout) chunks.push(chunk);
      const output = Buffer.concat(chunks); let count = 0, offset = 0;
      while (offset + 4 <= output.length) {
        const size = output.readUInt32LE(offset); if (offset + size + 4 > output.length) break;
        offset += size + 4; count++;
      }
      assert.ok(Date.now() - started >= 4500, 'output stress must terminate on the watchdog deadline');
      assert.ok(count > 1 && count < 4001, 'undrained output prevented completion of the queued responses');
    }
  } finally { clearTimeout(timeout); child.kill(); }
}
test('real guardian accepts handshake/status and native stop without any input authority', async () => {
  const guardian = await openGuardian({ binary, manifestFile, probe: true });
  try {
    assert.deepEqual(await guardian.status(), { state: 'inactive', nativeInput: false, authority: 'none' });
    assert.equal((await guardian.stop()).state, 'terminated');
    await assert.rejects(guardian.status());
  } finally { guardian.close(); }
});
test('native crypto validates issuance but cannot turn a valid signed grant into authority', async () => {
  const guardian = await openGuardian({ binary, manifestFile, probe: true });
  const pair = generateKeyPairSync('ed25519'), now = Date.now();
  const grant = { ...require('./fixtures/assist-control-v1.json').grants.valid[1].signed.grant,
    issuedAt: now - 1000, expiresAt: now + 60_000 };
  const canonical = Object.fromEntries(Object.keys(grant).sort().map(key => [key, grant[key]]));
  const signed = { grant, signature: sign(null, Buffer.from(JSON.stringify(canonical)), pair.privateKey).toString('base64url') };
  const issuer = pair.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64url');
  const expected = { session: grant.session, hostAccount: grant.hostAccount, hostDevice: grant.hostDevice,
    guestAccount: grant.guestAccount, guestDevice: grant.guestDevice, guestTransport: grant.guestTransport,
    endpoint: grant.endpoint, sessionDeadline: now + 60_000, minimumGeneration: 1 };
  try {
    assert.deepEqual(await guardian.verifyIssuance(signed, issuer, expected), { signatureValid: true, authorized: false });
    for (const [envelope, key, binding] of [
      [{ ...signed, grant: { ...grant, capability: 'control_keyboard' } }, issuer, expected],
      [signed, randomBytes(32).toString('base64url'), expected],
      [signed, issuer, { ...expected, endpoint: 'different_endpoint' }],
      [signed, issuer, { ...expected, minimumGeneration: 2 }],
    ]) {
      const result = await guardian.verifyIssuance(envelope, key, binding);
      assert.equal(result.signatureValid, false); assert.equal(result.authorized, false);
    }
  } finally { guardian.close(); }
});
test('actual pipes reject wrong parent, malformed/oversized/incomplete frames, replay and probe signing', async () => {
  await terminated(null, { pid: process.pid + 1 });
  await terminated(null, { mode: '--guardian' }); // Node is not the packaged Palisade parent.
  await terminated(Buffer.from([255, 255, 255, 255]));
  await terminated(Buffer.from([4, 0, 0, 0, 123]), { end: true });
  await terminated(frame('{"sequence":0,"sequence":0,"command":{"type":"status"}}'));
  await terminated(frame({ type: 'status' }));
  await terminated(Buffer.concat([frame(hello), frame({ type: 'status' }, 0)]));
  await terminated(Buffer.concat([frame(hello), frame({ type: 'enrollment-challenge', challenge: hello.challenge }, 1)]));
  await terminated(null, { end: true });
});
test('helper terminates after its actual parent exits', async () => {
  const script = `const {spawn}=require('node:child_process');
    const child=spawn(process.argv[1],['--probe',String(process.pid)],{windowsHide:true,stdio:['pipe','pipe','ignore']});
    child.stdin.on('error',()=>{}); child.on('exit',()=>process.exit(2));
    const data=Buffer.from(JSON.stringify({sequence:0,command:{type:'hello',protocol:'palisade.assist.guardian',version:1,challenge:process.argv[2]}}));
    const prefix=Buffer.alloc(4);prefix.writeUInt32LE(data.length);
    child.stdout.once('data',()=>process.stdout.write(String(child.pid),()=>process.exit(0)));
    child.stdin.write(Buffer.concat([prefix,data]));`;
  const parent = spawn(process.execPath, ['-e', script, binary, hello.challenge], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let result = ''; parent.stdout.on('data', chunk => { result += chunk; }); parent.stderr.resume();
  const timer = setTimeout(() => parent.kill(), 5000);
  try { assert.equal((await once(parent, 'exit'))[0], 0); } finally { clearTimeout(timer); }
  const pid = Number(result); assert.ok(Number.isInteger(pid) && pid > 0);
  const deadline = Date.now() + 4000;
  while (Date.now() < deadline) {
    try { process.kill(pid, 0); } catch (error) { if (error.code === 'ESRCH') return; throw error; }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.fail('orphaned helper remained running');
});
test('independent watchdog terminates a silent or blocked parent pipe', async () => {
  await terminated(null, { idle: true });
  await terminated(Buffer.from([10]), { idle: true });
  await terminated(frame(hello), { idle: true });
  const blockedOutput = [frame(hello)];
  for (let sequence = 1; sequence <= 4000; sequence++) blockedOutput.push(frame({ type: 'status' }, sequence));
  await terminated(Buffer.concat(blockedOutput), { idle: true, drain: false });
});
