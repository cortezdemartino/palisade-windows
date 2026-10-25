'use strict';
// Internal main-process diagnostics only. No page bridge or production startup imports this module.
const fs = require('node:fs/promises');
const path = require('node:path');
const { createHash, randomBytes } = require('node:crypto');
const { spawn } = require('node:child_process');
const MAX_FRAME = 16_384;
const FILENAME = 'Palisade-Assist.exe';

async function verifyGuardian(binary, manifestFile) {
  const manifest = JSON.parse(await fs.readFile(manifestFile, 'utf8'));
  if (Object.keys(manifest).sort().join(',') !== 'filename,protocol,sha256,size,version' ||
      manifest.protocol !== 'palisade.assist.guardian' || manifest.version !== 1 || manifest.filename !== FILENAME ||
      !Number.isSafeInteger(manifest.size) || manifest.size <= 0 || manifest.size > 20 * 1024 * 1024 ||
      !/^[a-f0-9]{64}$/.test(manifest.sha256)) throw new Error('Invalid Assist helper manifest');
  const metadata = await fs.lstat(binary);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size !== manifest.size) throw new Error('Assist helper size mismatch');
  const bytes = await fs.readFile(binary);
  if (createHash('sha256').update(bytes).digest('hex') !== manifest.sha256) throw new Error('Assist helper integrity failure');
  return manifest;
}

async function openGuardian({ binary, manifestFile, probe = false }) {
  await verifyGuardian(binary, manifestFile);
  const child = spawn(binary, [probe ? '--probe' : '--guardian', String(process.pid)], {
    windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'], shell: false,
  });
  let buffered = Buffer.alloc(0), sequence = 0, pending = null, ended = false;
  let heartbeat, reason = '';
  const fail = error => {
    if (ended) return;
    ended = true; clearInterval(heartbeat);
    if (pending) { clearTimeout(pending.timer); pending.reject(error); pending = null; }
    child.stdin.destroy(); child.stdout.destroy(); child.stderr.destroy(); child.kill();
  };
  child.on('error', () => fail(new Error('Assist helper launch failed')));
  child.stderr.on('data', chunk => {
    if (reason.length + chunk.length <= 128) reason += chunk.toString('utf8');
  });
  child.stderr.on('error', () => {});
  child.on('exit', () => fail(new Error('Assist helper ended' +
    (/^guardian: [a-z-]+\s*$/.test(reason) ? ' (' + reason.trim().slice(10) + ')' : ''))));
  child.stdin.on('error', () => fail(new Error('Assist helper pipe failed')));
  child.stdout.on('error', () => fail(new Error('Assist helper pipe failed')));
  child.stdout.on('data', chunk => {
    if (ended) return;
    // Only one bounded response can be outstanding. Never retain unbounded child output.
    if (buffered.length + chunk.length > MAX_FRAME + 4) return fail(new Error('Assist helper response too large'));
    buffered = Buffer.concat([buffered, chunk]);
    if (buffered.length < 4) return;
    const size = buffered.readUInt32LE();
    if (!size || size > MAX_FRAME) return fail(new Error('Assist helper framing failure'));
    if (buffered.length < size + 4) return;
    if (!pending || buffered.length !== size + 4) return fail(new Error('Unexpected Assist helper response'));
    let message;
    try { message = JSON.parse(buffered.subarray(4).toString('utf8')); } catch { return fail(new Error('Invalid Assist helper response')); }
    if (!message || Object.keys(message).sort().join(',') !== 'result,sequence' || message.sequence !== pending.sequence ||
        !message.result || typeof message.result !== 'object' || Array.isArray(message.result)) return fail(new Error('Invalid Assist helper response'));
    const current = pending; pending = null; buffered = Buffer.alloc(0);
    clearTimeout(current.timer); current.resolve(message.result);
  });
  const request = command => new Promise((resolve, reject) => {
    if (ended || pending || sequence > 0xffffffff) return reject(new Error('Assist helper unavailable'));
    const bytes = Buffer.from(JSON.stringify({ sequence, command }));
    if (bytes.length > MAX_FRAME) return reject(new Error('Assist helper request too large'));
    const prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length);
    pending = { sequence: sequence++, resolve, reject, timer: setTimeout(() => fail(new Error('Assist helper timed out')), 2500) };
    child.stdin.write(Buffer.concat([prefix, bytes]));
  });
  try {
    const challenge = randomBytes(32).toString('base64url');
    const hello = await request({ type: 'hello', protocol: 'palisade.assist.guardian', version: 1, challenge });
    if (hello.protocol !== 'palisade.assist.guardian' || hello.version !== 1 || hello.challenge !== challenge ||
        hello.mode !== (probe ? 'probe' : 'guardian') || hello.nativeInput !== false) throw new Error('Assist helper handshake failed');
    heartbeat = setInterval(() => {
      if (!pending && !ended) request({ type: 'status' }).catch(fail);
    }, 1000); heartbeat.unref();
    return Object.freeze({
      // No generic command API, enrollment signer, file path, clipboard or input operation.
      status: () => request({ type: 'status' }),
      verifyIssuance: (signed, issuer, expected) => request({ type: 'verify-grant', signed, issuer, expected }),
      stop: async () => { clearInterval(heartbeat); try { return await request({ type: 'stop' }); } finally { fail(new Error('Assist stopped')); } },
      close: () => fail(new Error('Assist helper closed')),
      pid: child.pid,
    });
  } catch (error) { fail(error); throw error; }
}

function packagedGuardianPaths(resourcesPath) {
  return { binary: path.join(resourcesPath, 'assist', FILENAME),
    manifestFile: path.join(resourcesPath, 'app.asar', 'src', 'assist', 'guardian-integrity.json') };
}
module.exports = { openGuardian, verifyGuardian, packagedGuardianPaths, FILENAME };
