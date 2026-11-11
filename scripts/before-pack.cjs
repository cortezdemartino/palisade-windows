'use strict';
const bundleManager = require('./bundle-manager.cjs');
const { buildAssist } = require('./build-assist.cjs');
const { buildAssistPreload } = require('./build-assist-preload.cjs');
const { buildUpdate } = require('./build-update.cjs');
module.exports = async context => {
  const { qualification } = require('../src/assist/qualification.cjs');
  if (qualification && (process.env.PALISADE_ASSIST_QUALIFICATION_BUILD !== '1' ||
      context.packager.appInfo.id !== 'org.onyxops.palisade.qualification.' + qualification.namespace)) {
    throw Error('Qualification profiles require the separate unsigned qualification build');
  }
  require('./release-notes.cjs').loadReleaseDraft(require('../package.json').version);
  await bundleManager(context);
  await buildAssist();
  await buildAssistPreload();
  await buildUpdate();
};
