'use strict';

/** The only top-level renderer origin trusted by the public desktop edition. */
const PALISADE_URL = 'https://palisade.onyxops.org/';
const PALISADE_ORIGIN = new URL(PALISADE_URL).origin;

function parseUrl(value) {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

/** HTTPS-only, exact origin; never accept credentials or lookalike domains. */
function isPalisadeUrl(value) {
  const url = parseUrl(value);
  return Boolean(
    url &&
      url.protocol === 'https:' &&
      url.origin === PALISADE_ORIGIN &&
      !url.username &&
      !url.password
  );
}

/** External links must be explicitly approved and are never rendered inside Electron. */
function normaliseExternalUrl(value) {
  if (typeof value !== 'string' || value.length > 2048 || /[\u0000-\u001f\u007f]/.test(value)) {
    return null;
  }
  const url = parseUrl(value);
  if (!url || url.protocol !== 'https:' || url.username || url.password) return null;
  if (isPalisadeUrl(url.href)) return null;
  return url.href;
}

/** No device classes except Palisade-origin microphone and notifications. */
function mayGrantPermission(permission, requestingUrl, mediaTypes) {
  if (!isPalisadeUrl(requestingUrl)) return false;
  if (permission === 'notifications') return true;
  if (permission !== 'media') return false;
  return (
    Array.isArray(mediaTypes) &&
    mediaTypes.length === 1 &&
    mediaTypes[0] === 'audio'
  );
}

module.exports = {
  PALISADE_URL,
  PALISADE_ORIGIN,
  isPalisadeUrl,
  normaliseExternalUrl,
  mayGrantPermission,
};
