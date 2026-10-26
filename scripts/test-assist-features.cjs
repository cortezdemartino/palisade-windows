'use strict';
// Compile sensitive adapters, then exercise only non-authorizing probe modes.
const fs = require('node:fs/promises');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { spawnSync } = require('node:child_process');
async function main() {
  const root=path.join(__dirname,'..');
  const build=spawnSync('cargo',['build','--locked','--manifest-path','native/assist/Cargo.toml','--all-features'],{cwd:root,windowsHide:true,stdio:'inherit',shell:false});
  if(build.error||build.status!==0)throw Error('Feature compilation failed');
  const binary=path.join(root,'native/assist/target/debug/palisade-assist-guardian.exe');
  const bytes=await fs.readFile(binary),directory=path.join(root,'out/assist-feature-probe');await fs.mkdir(directory,{recursive:true});
  const manifestFile=path.join(directory,'integrity.json');
  await fs.writeFile(manifestFile,JSON.stringify({protocol:'palisade.assist.guardian',version:1,filename:'Palisade-Assist.exe',size:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')}));
  const tests=spawnSync(process.execPath,['--test','tests/assist-guardian-process.native.cjs'],{cwd:root,windowsHide:true,stdio:'inherit',shell:false,
    env:{...process.env,PALISADE_ASSIST_TEST_BINARY:binary,PALISADE_ASSIST_TEST_MANIFEST:manifestFile}});
  if(tests.error||tests.status!==0)throw Error('Compiled-feature probe tests failed');
}
main().catch(error=>{console.error(error.message);process.exitCode=1;});
