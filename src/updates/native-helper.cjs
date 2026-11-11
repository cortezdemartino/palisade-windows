'use strict';
// Main-process only. No executable path or helper protocol is exposed to hosted content.
const fs = require('node:fs/promises');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { spawn } = require('node:child_process');
const { StringDecoder } = require('node:string_decoder');
const FILENAME = 'Palisade-Update.exe';
const PROTOCOL = 'palisade.update.helper';
const MAX_OUTPUT = 4096;
const UUID = /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/;
const DIGEST = /^[a-f0-9]{64}$/;
async function verifyHelper(binary, manifestFile, version) {
  const bytes = Buffer.isBuffer(manifestFile) ? manifestFile : await fs.readFile(manifestFile);
  if (bytes.length > 2048) throw Error('Invalid updater integrity record.');
  const manifest = JSON.parse(bytes);
  if (Object.keys(manifest).sort().join(',') !== 'filename,protocol,schema,sha256,size,version' ||
      manifest.protocol !== PROTOCOL || manifest.schema !== 1 || manifest.filename !== FILENAME || manifest.version !== version ||
      !Number.isSafeInteger(manifest.size) || manifest.size <= 0 || manifest.size > 32 * 1024 * 1024 ||
      !/^[a-f0-9]{64}$/.test(manifest.sha256)) throw Error('Invalid updater integrity record.');
  const metadata = await fs.lstat(binary);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size !== manifest.size) throw Error('Updater integrity mismatch.');
  const file = await fs.open(binary, 'r');
  try {
    const hash = createHash('sha256'); let size = 0;
    for await (const chunk of file.createReadStream({ autoClose: false })) {
      size += chunk.length;
      if (size > manifest.size) throw Error('Updater integrity mismatch.');
      hash.update(chunk);
    }
    if (size !== manifest.size || hash.digest('hex') !== manifest.sha256) throw Error('Updater integrity mismatch.');
  } finally { await file.close(); }
  return manifest;
}
function packagedPaths(resources) {
  return { binary: path.join(resources, 'update', FILENAME), manifestFile: path.join(resources, 'app.asar/src/updates/helper-integrity.json') };
}

// Keep stdin open: native verification also checks that its actual authenticated parent retains the request.
function exchange(binary, args, { signal, timeout = 10000, spawnFn = spawn } = {}) {
  signal?.throwIfAborted();
  const child = spawnFn(binary, args, { shell: false, detached: true, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  const decoder = new StringDecoder('utf8');
  let output = '', errors = '', ended = false;
  const response = new Promise((resolve, reject) => {
    const finish = (error, value) => {
      if (ended) return; ended = true; clearTimeout(timer); signal?.removeEventListener('abort', canceled);
      // Never terminate the helper: after commit it owns installer cleanup independently.
      child.stdin.end(); if (error) reject(error); else resolve(value);
    };
    const canceled = () => finish(Error('Update request canceled.'));
    const timer = setTimeout(() => finish(Error('Native updater did not respond.')), timeout);
    signal?.addEventListener('abort', canceled, { once: true });
    child.on('error', () => finish(Error('Native updater could not start.')));
    child.stdin.on('error', () => finish(Error('Native updater connection ended.')));
    child.stdout.on('error', () => finish(Error('Native updater connection ended.')));
    child.stderr.on('error', () => finish(Error('Native updater connection ended.')));
    child.stderr.on('data', chunk => {
      if (ended) return;
      errors += chunk.toString();
      if (errors.length > 256) finish(Error('Invalid updater response.'));
    });
    child.stdout.on('data', chunk => {
      if (ended) return;
      output += decoder.write(chunk);
      if (Buffer.byteLength(output) > MAX_OUTPUT) finish(Error('Invalid updater response.'));
    });
    child.once('close', code => {
      if (ended) return;
      output += decoder.end();
      if (code !== 0 || !output.endsWith('\n') || output.trim().includes('\n')) return finish(Error('Native updater refused the request.'));
      try { finish(null, JSON.parse(output)); } catch { finish(Error('Invalid updater response.')); }
    });
  });
  return response;
}
function exchangeApply(binary, pid, { stage, nonce, version, releaseSha256 }, { signal, onCommit, spawnFn = spawn } = {}) {
  signal?.throwIfAborted();
  const child = spawnFn(binary, ['--apply', String(pid), stage, nonce],
    { shell: false, detached: true, windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] });
  return new Promise((resolve, reject) => {
    let output = '', ended = false, sent = false;
    let timer = setTimeout(() => fail('Update verification timed out.'), 610000);
    function finish(error) {
      if (ended) return;
      ended = true;
      clearTimeout(timer);
      signal?.removeEventListener('abort', canceled);
      child.stdin.end();
      child.stdout.destroy();
      child.unref();
      if (error) reject(error); else resolve();
    }
    function fail(message) {
      finish(Object.assign(Error(message), { commitSent: sent }));
    }
    function canceled() { if (!sent) fail('Update restart canceled.'); }
    signal?.addEventListener('abort', canceled, { once: true });
    child.on('error', () => fail('The update helper could not start.'));
    child.stdin.on('error', () => fail('The update helper connection ended.'));
    child.stdout.on('error', () => fail('The update helper connection ended.'));
    child.once('close', () => fail('The update helper ended before acknowledging the restart.'));
    child.stdout.on('data', chunk => {
      if (ended) return;
      output += chunk.toString('utf8');
      if (Buffer.byteLength(output) > MAX_OUTPUT) return fail('Invalid restart response.');
      if (!output.includes('\n')) return;
      if (!output.endsWith('\n') || output.trim().includes('\n')) return fail('Invalid restart response.');
      let value;
      try { value = JSON.parse(output); } catch { return fail('Invalid restart response.'); }
      output = '';
      if (!value || value.stage !== stage || value.nonce !== nonce) return fail('Invalid restart response.');
      const fields = Object.keys(value).sort().join(',');
      if (!sent && value.state === 'ready' && fields === 'nonce,releaseSha256,stage,state,version' &&
          value.version === version && value.releaseSha256 === releaseSha256) {
        if (signal?.aborted) return canceled();
        try { onCommit?.(); } catch { return fail('Restart authorization could not be recorded.'); }
        sent = true;
        clearTimeout(timer);
        timer = setTimeout(() => fail('The update helper did not confirm the restart.'), 10000);
        child.stdin.write('PALISADE-UPDATE-COMMIT-V1 ' + nonce + '\n');
      } else if (sent && value.state === 'committed' && fields === 'nonce,stage,state') {
        finish();
      } else {
        fail('Invalid restart response.');
      }
    });
  });
}
function createNativeHelper({ resources, version, pid = process.pid, exchangeFn = exchange }) {
  const paths = packagedPaths(resources);
  const call = async (command, id, options = {}) => {
    if (!['status', 'stage', 'inspect', 'discard'].includes(command) || (command !== 'status' && !UUID.test(id)))
      throw Error('Invalid updater request.');
    await verifyHelper(paths.binary, paths.manifestFile, version);
    const value = await exchangeFn(paths.binary, ['--' + command, String(pid), ...(id ? [id] : [])], options);
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid updater response.');
    if (command === 'status') {
      if (Object.keys(value).sort().join(',') !== 'protocol,schema,state' || value.protocol !== PROTOCOL || value.schema !== 1 ||
          !['idle', 'updating'].includes(value.state)) throw Error('Invalid updater status.');
    } else if (value.stage !== id || value.state !== ({ stage: 'created', inspect: 'verified', discard: 'discarded' })[command]) {
      throw Error('Invalid updater response.');
    }
    if (command === 'stage' || command === 'inspect') {
      // This path remains main-only; native independently reopens the fixed known-folder UUID.
      if (typeof value.directory !== 'string' || !path.isAbsolute(value.directory) || path.basename(value.directory) !== id)
        throw Error('Invalid updater stage.');
    }
    if (command === 'inspect' && (typeof value.version !== 'string' || !/^[a-f0-9]{64}$/.test(value.releaseSha256)))
      throw Error('Invalid updater verification.');
    return value;
  };
  async function apply(verified, nonce, options) {
    if (verified?.state !== 'verified' || !UUID.test(verified.stage) || !DIGEST.test(verified.releaseSha256) ||
        !DIGEST.test(nonce) || !path.isAbsolute(verified.directory) || path.basename(verified.directory) !== verified.stage)
      throw Error('Verified update stage required.');
    const staged = path.join(verified.directory, FILENAME);
    await verifyHelper(staged, paths.manifestFile, version);
    return exchangeApply(staged, pid, { ...verified, nonce }, options);
  }
  return { status: options => call('status', undefined, options), stage: (id, options) => call('stage', id, options),
    inspect: (id, options) => call('inspect', id, { timeout: 610000, ...options }),
    discard: (id, options) => call('discard', id, options), apply };
}
module.exports = { FILENAME, PROTOCOL, UUID, verifyHelper, packagedPaths, exchange, exchangeApply, createNativeHelper };
