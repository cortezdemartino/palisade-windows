'use strict';

const { createHash, verify } = require('node:crypto');
const { signedBytes, verifyEnvelope } = require('./release.cjs');
const { resolveTrust } = require('./key-trust.cjs');

const NOTES_ASSET = 'release-notes-v1.json';
const NOTES_LIMIT = 64 * 1024;
const PRODUCT = 'org.onyxops.palisade';
const CATEGORIES = Object.freeze({ new: 'New', improved: 'Improved', fixed: 'Fixed',
  security: 'Security & Privacy', limitations: 'Known Limitations' });

function exact(value, fields) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      ![Object.prototype, null].includes(Object.getPrototypeOf(value)) ||
      Object.keys(value).sort().join(',') !== fields.slice().sort().join(',')) {
    throw new Error('Unexpected release-note fields.');
  }
}

function text(value, maximum, empty = false) {
  if (typeof value !== 'string' || value.length > maximum || (!empty && !value.trim()) ||
      value !== value.trim() || value !== value.normalize('NFC') ||
      /[\p{Cc}\p{Cf}\p{Cs}<>]/u.test(value) || /(?:[a-z][a-z\d+.-]*:\/\/|\]\s*\()/i.test(value)) {
    throw new Error('Release notes must contain bounded plain text without links or markup.');
  }
  return value;
}

/** Editorial input is intentionally separate from the signed installer payload. */
function normaliseDraft(source, expectedVersion = source?.version) {
  exact(source, ['schema', 'version', 'locale', 'title', 'summary', 'sections', 'compatibility']);
  if (source.schema !== 1 || typeof source.version !== 'string' ||
      !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(source.version) ||
      source.version.split('.').some((n) => !Number.isSafeInteger(Number(n))) || source.version !== expectedVersion ||
      typeof source.locale !== 'string' || !/^[a-z]{2,3}(?:-[A-Z]{2})?$/.test(source.locale)) {
    throw new Error('Release-note version, locale or schema is invalid.');
  }
  if (!Array.isArray(source.sections) || source.sections.length > 5 ||
      !Array.isArray(source.compatibility) || source.compatibility.length > 6) {
    throw new Error('Too many release-note sections or compatibility entries.');
  }
  const seen = new Set();
  const sections = source.sections.map((section) => {
    exact(section, ['category', 'items']);
    if (!Object.hasOwn(CATEGORIES, section.category) || seen.has(section.category) ||
        !Array.isArray(section.items) || !section.items.length || section.items.length > 12) {
      throw new Error('Invalid or duplicate release-note category.');
    }
    seen.add(section.category);
    return { category: section.category, items: section.items.map((item) => text(item, 600)) };
  });
  const result = { schema: 1, version: source.version, locale: source.locale,
    title: text(source.title, 120, true), summary: text(source.summary, 600), sections,
    compatibility: source.compatibility.map((item) => text(item, 400)) };
  if (Buffer.byteLength(JSON.stringify(result)) > NOTES_LIMIT - 4096) throw new Error('Release notes are too large.');
  return result;
}

function bindDraft(draft, release) {
  if (release.schema !== 3) throw new Error('Release notes require a verified v3 release.');
  const editorial = normaliseDraft(draft, release.version);
  return { schema: 1, product: PRODUCT, platform: 'win32-x64', channel: release.channel,
    version: release.version, publishedAt: release.publishedAt,
    releaseSha256: createHash('sha256').update(signedBytes(release)).digest('hex'),
    locale: editorial.locale, title: editorial.title, summary: editorial.summary,
    sections: editorial.sections, compatibility: editorial.compatibility };
}

function normaliseNotes(source, release) {
  exact(source, ['schema', 'product', 'platform', 'channel', 'version', 'publishedAt', 'releaseSha256',
    'locale', 'title', 'summary', 'sections', 'compatibility']);
  const result = bindDraft({ schema: source.schema, version: source.version, locale: source.locale,
    title: source.title, summary: source.summary, sections: source.sections, compatibility: source.compatibility }, release);
  for (const field of ['product', 'platform', 'channel', 'version', 'publishedAt', 'releaseSha256']) {
    if (source[field] !== result[field]) throw new Error('Release notes do not match the authorized release.');
  }
  return result;
}

function notesBytes(payload, release) {
  return Buffer.from('PALISADE-DESKTOP-RELEASE-NOTES-V1\n' + JSON.stringify(normaliseNotes(payload, release)));
}

function verifyNotes(envelope, releaseEnvelope, root, installedTransitions = []) {
  const release = verifyEnvelope(releaseEnvelope, root, installedTransitions);
  exact(envelope, ['payload', 'signature']);
  if (typeof envelope.signature !== 'string' || !/^[A-Za-z0-9+/]{86}==$/.test(envelope.signature) ||
      Buffer.byteLength(JSON.stringify(envelope)) > NOTES_LIMIT) throw new Error('Invalid release-note signature or size.');
  const payload = normaliseNotes(envelope.payload, release);
  const key = resolveTrust(root, releaseEnvelope.transitions, release.version);
  if (!verify(null, notesBytes(payload, release), key, Buffer.from(envelope.signature, 'base64'))) {
    throw new Error('Release-note signature verification failed.');
  }
  return payload;
}

/** Text-only presentation shared by the desktop and Manager. Never interprets Markdown/HTML. */
function noteView(payload) {
  return { version: payload.version, publishedAt: payload.publishedAt, locale: payload.locale,
    title: payload.title || `What's New in Palisade ${payload.version}`, summary: payload.summary,
    sections: payload.sections.map(({ category, items }) => ({ title: CATEGORIES[category], items: [...items] })),
    compatibility: [...payload.compatibility] };
}

module.exports = { NOTES_ASSET, NOTES_LIMIT, PRODUCT, CATEGORIES, normaliseDraft, bindDraft,
  normaliseNotes, notesBytes, verifyNotes, noteView };
