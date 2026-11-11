'use strict';
// Owner review and native/cross-client qualification are required before production activation.
const { qualification } = require('./qualification.cjs');
module.exports = Object.freeze({ captureEnabled: false, nativeEndpointEnabled: qualification !== null, qualification });
