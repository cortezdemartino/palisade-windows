'use strict';
const bundleManager = require('./bundle-manager.cjs');
const { buildAssist, qualificationFeatures } = require('./build-assist.cjs');
const { buildAssistPreload } = require('./build-assist-preload.cjs');
const { buildUpdate } = require('./build-update.cjs');
module.exports = async context => {
  const { qualification } = require('../src/assist/qualification.cjs');
  const qualified = qualification !== null;
  if (qualified && (process.env.PALISADE_ASSIST_QUALIFICATION_BUILD !== '1' ||
      context.packager.appInfo.id !== 'org.onyxops.palisade.qualification.' + qualification.namespace)) {
    throw Error('Qualification profiles require the separate unsigned qualification build');
  }
  if (!qualified && (process.env.PALISADE_ASSIST_QUALIFICATION_BUILD === '1' || process.env.PALISADE_ASSIST_QUALIFICATION_FEATURES)) {
    throw Error('Production packaging refuses qualification feature overrides');
  }
  if (qualified && (context.packager.appInfo.productName !== 'Palisade Assist Qualification' || context.electronPlatformName !== 'win32' || context.arch !== 1 ||
      !Array.isArray(context.targets) || context.targets.some(target => target.name !== 'dir') ||
      context.packager.platformSpecificBuildOptions?.signAndEditExecutable !== false)) {
    throw Error('Qualification builds must be unsigned Windows x64 unpacked directories');
  }
  const features = qualified ? qualificationFeatures(process.env.PALISADE_ASSIST_QUALIFICATION_FEATURES || '') : [];
  require('./release-notes.cjs').loadReleaseDraft(require('../package.json').version);
  await bundleManager(context);
  await buildAssist({ qualificationBuild: qualified, features });
  await buildAssistPreload();
  await buildUpdate();
};
