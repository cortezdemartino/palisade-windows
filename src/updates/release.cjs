'use strict';

const { createHash, createPublicKey, verify, timingSafeEqual } = require('node:crypto');
const { createReadStream } = require('node:fs');
const { stat } = require('node:fs/promises');

const OWNER = 'cortezdemartino';
const REPOSITORY = 'palisade-windows';
const RELEASES_API = `https://api.github.com/repos/${OWNER}/${REPOSITORY}/releases/latest`;
const RELEASE_BASE = `https://github.com/${OWNER}/${REPOSITORY}/releases/download/`;
const MANIFEST_BYTES_LIMIT = 16 * 1024;
const API_BYTES_LIMIT = 256 * 1024;
const MAX_INSTALLER_BYTES = 1024 * 1024 * 1024;
const ASSET_HOSTS = new Set([
  'github.com',
  'release-assets.githubusercontent.com',
  'objects.githubusercontent.com',
]);

function plainObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value) &&
    (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);
}

function parseVersion(value) {
  if (typeof value !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value)) {
    throw new Error('Release version must use numeric major.minor.patch.');
  }
  const parts = value.split('.').map(Number);
  if (parts.some((n) => !Number.isSafeInteger(n))) throw new Error('Release version is too large.');
  return parts;
}

function compareVersions(a, b) {
  function semver(value) {
    if (typeof value !== 'string') throw new Error('Invalid semantic version.');
    const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(value);
    if (!match) throw new Error('Invalid semantic version.');
    const pre = match[4]?.split('.') || [];
    if (pre.some((p) => /^\d+$/.test(p) && p.length > 1 && p[0] === '0')) throw new Error('Invalid prerelease.');
    return { core: parseVersion(match.slice(1, 4).join('.')), pre };
  }
  const left = semver(a), right = semver(b);
  const x = left.core, y = right.core;
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i] ? 1 : -1;
  if (!left.pre.length || !right.pre.length) return left.pre.length ? -1 : right.pre.length ? 1 : 0;
  for (let i = 0; i < Math.max(left.pre.length, right.pre.length); i++) {
    const l = left.pre[i], r = right.pre[i];
    if (l === r) continue;
    if (l === undefined || r === undefined) return l === undefined ? -1 : 1;
    const ln = /^\d+$/.test(l), rn = /^\d+$/.test(r);
    if (ln && rn) return BigInt(l) > BigInt(r) ? 1 : -1;
    if (ln !== rn) return ln ? -1 : 1;
    return l > r ? 1 : -1;
  }
  return 0;
}

function normalisePayload(source) {
  if (!plainObject(source)) throw new Error('Invalid release payload.');
  const keys = ['channel', 'filename', 'publishedAt', 'schema', 'sha512', 'size', 'version'];
  if (source.schema === 2) keys.push('sha256');
  if (Object.keys(source).sort().join(',') !== keys.sort().join(',')) {
    throw new Error('Unexpected release fields.');
  }
  if (![1, 2].includes(source.schema) || source.channel !== 'stable') throw new Error('Invalid release channel or schema.');
  if (source.schema === 2 && (typeof source.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(source.sha256))) {
    throw new Error('Installer SHA-256 is invalid.');
  }
  parseVersion(source.version);
  const filename = `Palisade-Setup-${source.version}-x64.exe`;
  if (source.filename !== filename) throw new Error('Installer filename must match the release version.');
  if (!Number.isSafeInteger(source.size) || source.size <= 0 || source.size > MAX_INSTALLER_BYTES) {
    throw new Error('Installer size is invalid.');
  }
  if (typeof source.sha512 !== 'string' || !/^[A-Za-z0-9+/]{86}==$/.test(source.sha512) ||
      Buffer.from(source.sha512, 'base64').length !== 64) {
    throw new Error('Installer SHA-512 is invalid.');
  }
  if (typeof source.publishedAt !== 'string' ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(source.publishedAt) ||
      !Number.isFinite(Date.parse(source.publishedAt)) || new Date(source.publishedAt).toISOString() !== source.publishedAt) {
    throw new Error('Release timestamp is invalid.');
  }
  return { schema: source.schema, channel: 'stable', version: source.version, filename, sha512: source.sha512,
    size: source.size, publishedAt: source.publishedAt, ...(source.schema === 2 ? { sha256: source.sha256 } : {}) };
}

function signedBytes(source) {
  return Buffer.from(`PALISADE-DESKTOP-RELEASE-V${source.schema}\n` + JSON.stringify(normalisePayload(source)), 'utf8');
}

function verifyEnvelope(envelope, publicKeyPem) {
  if (!plainObject(envelope) || Object.keys(envelope).sort().join(',') !== 'payload,signature') {
    throw new Error('Invalid signed release envelope.');
  }
  const payload = normalisePayload(envelope.payload);
  if (typeof envelope.signature !== 'string' || !/^[A-Za-z0-9+/]{86}==$/.test(envelope.signature)) {
    throw new Error('Release manifest signature is missing.');
  }
  const signature = Buffer.from(envelope.signature, 'base64');
  const key = publicKeyPem?.type === 'public' ? publicKeyPem : createPublicKey(publicKeyPem);
  if (key.asymmetricKeyType !== 'ed25519' || signature.length !== 64 ||
      !verify(null, signedBytes(payload), key, signature)) {
    throw new Error('Release manifest signature verification failed.');
  }
  return payload;
}

async function readLimited(response, maximum) {
  if (!response.ok || !response.body) throw new Error('Release download failed.');
  const contentLength = response.headers.get('content-length');
  if (contentLength !== null && Number(contentLength) > maximum) throw new Error('Release response too large.');
  const parts = [];
  let length = 0;
  for await (const part of response.body) {
    length += part.length;
    if (length > maximum) {
      throw new Error('Release response too large.');
    }
    parts.push(part);
  }
  return JSON.parse(Buffer.concat(parts).toString('utf8'));
}

function releaseAssetUrl(version, filename) {
  parseVersion(version);
  if (!['release.json', 'release-v2.json', `Palisade-Setup-${version}-x64.exe`].includes(filename)) {
    throw new Error('Unrecognized release asset.');
  }
  return `${RELEASE_BASE}v${version}/${filename}`;
}

/** Follow only standard GitHub release-asset redirects; never accept an arbitrary download URL. */
async function fetchAsset(url, { fetchFn = fetch, signal = AbortSignal.timeout(30000) } = {}) {
  let target = url;
  for (let redirects = 0; redirects < 6; redirects++) {
    const parsed = new URL(target);
    if (parsed.protocol !== 'https:' || parsed.port || parsed.username || parsed.password || !ASSET_HOSTS.has(parsed.hostname)) {
      throw new Error('Release asset redirected outside GitHub.');
    }
    const response = await fetchFn(target, { redirect: 'manual', signal,
      headers: { 'User-Agent': 'PalisadeDesktop-Updater', Accept: 'application/octet-stream' } });
    if ([301, 302, 303, 307, 308].includes(response.status)) {
      const location = response.headers.get('location');
      if (!location) throw new Error('Release asset redirect has no destination.');
      await response.body?.cancel?.();
      target = new URL(location, target).href;
      continue;
    }
    if (!response.ok) throw new Error(`Release asset HTTP ${response.status}.`);
    return response;
  }
  throw new Error('Too many GitHub release redirects.');
}

async function findLatestRelease(fetchFn = fetch, details = false) {
  const response = await fetchFn(RELEASES_API, {
    redirect: 'error', signal: AbortSignal.timeout(15000),
    headers: { 'User-Agent': 'PalisadeDesktop-Updater', Accept: 'application/vnd.github+json' },
  });
  if (!response.ok || response.url !== RELEASES_API) throw new Error('GitHub release metadata unavailable.');
  const info = await readLimited(response, API_BYTES_LIMIT);
  if (!plainObject(info) || info.draft !== false || info.prerelease !== false ||
      typeof info.tag_name !== 'string' || !info.tag_name.startsWith('v')) {
    throw new Error('Invalid stable GitHub release.');
  }
  const version = info.tag_name.slice(1);
  parseVersion(version);
  if (!Array.isArray(info.assets) || !info.assets.every((asset) => plainObject(asset))) {
    throw new Error('Release assets missing.');
  }
  const names = info.assets.map((asset) => asset.name);
  if (!names.includes('release.json') || !names.includes(`Palisade-Setup-${version}-x64.exe`)) {
    throw new Error('Required release assets missing.');
  }
  return details ? { version, notes: typeof info.body === 'string' ? info.body.slice(0, 8000) : '' } : version;
}

async function fetchRelease(fetchFn, publicKeyPem, installedVersion) {
  const { version, notes } = await findLatestRelease(fetchFn, true);
  if (compareVersions(version, installedVersion) <= 0) return null;
  const response = await fetchAsset(releaseAssetUrl(version, 'release-v2.json'), { fetchFn });
  const envelope = await readLimited(response, MANIFEST_BYTES_LIMIT);
  const payload = verifyEnvelope(envelope, publicKeyPem);
  if (payload.schema !== 2) throw new Error('SHA-256 signed manifest required.');
  if (payload.version !== version || compareVersions(payload.version, installedVersion) <= 0) {
    throw new Error('Signed manifest version differs from the GitHub release.');
  }
  Object.defineProperty(payload, 'releaseNotes', { value: notes });
  return payload;
}

async function verifyDownloadedFile(filename, manifest) {
  let details;
  try { details = await stat(filename); } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
  if (!details.isFile() || details.size !== manifest.size) return false;
  const hash = createHash('sha512');
  const sha256 = createHash('sha256');
  for await (const bytes of createReadStream(filename)) { hash.update(bytes); sha256.update(bytes); }
  const actual = hash.digest();
  const expected = Buffer.from(manifest.sha512, 'base64');
  return actual.length === expected.length && timingSafeEqual(actual, expected) &&
    (manifest.schema !== 2 || sha256.digest('hex') === manifest.sha256);
}

module.exports = {
  RELEASES_API, RELEASE_BASE, MAX_INSTALLER_BYTES, compareVersions, normalisePayload,
  signedBytes, verifyEnvelope, readLimited, releaseAssetUrl, fetchAsset, findLatestRelease,
  fetchRelease, verifyDownloadedFile,
};
