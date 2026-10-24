'use strict';
const { isPalisadeUrl, mayGrantPermission } = require('./security-policy.cjs');

function installSessionPolicy(profile, getContents, capture = null) {
  function trustedRequester(requester, details) {
    const contents = getContents();
    if (!contents || requester !== contents || contents.isDestroyed() ||
        !isPalisadeUrl(contents.getURL()) || details?.isMainFrame !== true) return false;
    return ['requestingUrl', 'securityOrigin', 'embeddingOrigin'].every(field =>
      details[field] === undefined || isPalisadeUrl(details[field]));
  }

  profile.webRequest.onBeforeRequest({ urls: ['*://*/*'] }, (details, callback) => {
    callback({ cancel: details.resourceType === 'mainFrame' && !isPalisadeUrl(details.url) });
  });
  profile.setPermissionRequestHandler((contents, permission, callback, details) => {
    callback(trustedRequester(contents, details) && typeof details.requestingUrl === 'string' &&
      (mayGrantPermission(permission, contents.getURL(), details.mediaTypes) ||
        (permission === 'display-capture' && capture?.available() === true)));
  });
  profile.setPermissionCheckHandler((contents, permission, requestingOrigin, details) => {
    if (!trustedRequester(contents, details) || !isPalisadeUrl(requestingOrigin)) return false;
    return (permission === 'display-capture' && capture?.available() === true) || permission === 'notifications' || (permission === 'media' && details.mediaType === 'audio');
  });
  // Capture stays denied unless the separately gated controller is available.
  profile.setDisplayMediaRequestHandler((request, callback) => capture?.available() === true ? capture.handle(request, callback) : callback(null), { useSystemPicker: false });
  profile.setDevicePermissionHandler(() => false);
}

module.exports = { installSessionPolicy };
