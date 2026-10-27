"use strict";
// Real isolated Electron documents, fake coordinator. No capture, native input,
// enrollment storage, network service or personal clipboard is used.
const { app, BrowserWindow } = require("electron");
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const root =
  process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, "..");
const { installNativeBridge } = require(
  path.join(root, "src/assist/native-bridge.cjs"),
);
const fixtures = require("../tests/fixtures/assist-native-v1.json");
app.disableHardwareAcceleration();
app.setPath(
  "userData",
  path.join(__dirname, "../out/assist-native-bridge-profile"),
);
let win,
  bridge,
  phase = "startup",
  mode = "idle",
  stops = 0,
  savedScope;
const blocked = [],
  scopes = [];
const state = {
  endpoint: { enrolled: false, endpointId: null, pin: "none" },
  capture: { operation: null, state: "idle", trackSid: null },
  controller: { session: null, state: "idle" },
};
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const deadline = setTimeout(() => {
  console.error("Native bridge fixture timeout during", phase);
  app.exit(1);
}, 40000);
const message = (op) =>
  `JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op:${JSON.stringify(op)}})`;
app
  .whenReady()
  .then(async () => {
    win = new BrowserWindow({
      show: false,
      webPreferences: {
        partition: "assist-native-bridge-smoke",
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        preload: path.join(root, "src/assist/hosted-preload.cjs"),
        additionalArguments: ["--palisade-assist-native"],
      },
    });
    const contents = win.webContents;
    const evaluate = (expression) =>
      contents.executeJavaScript("(async()=>(" + expression + "))()");
    contents.session.setPermissionRequestHandler(
      (_contents, _permission, callback) => callback(false),
    );
    contents.session.setPermissionCheckHandler(() => false);
    contents.session.protocol.handle(
      "https",
      () =>
        new Response(
          "<!doctype html><title>Native bridge fixture</title><body>Owned synthetic document</body>",
          { headers: { "Content-Type": "text/html" } },
        ),
    );
    bridge = installNativeBridge(contents, {
      enabled: true,
      dispatch: async (request, scope) => {
        if (request.op === "hello")
          return {
            version: 1,
            features: {
              capture: false,
              clipboard: false,
              files: false,
              input: false,
            },
          };
        if (request.op === "capture.start") {
          savedScope = scope;
          return { operation: request.operation };
        }
        if (mode === "block") {
          scopes.push(scope);
          return await new Promise((resolve) => blocked.push(resolve));
        }
        return state;
      },
      stopAll: () => {
        stops++;
      },
    });
    await contents.loadURL("https://palisade.onyxops.org/native-fixture");
    phase = "surface and frame boundaries";
    assert.equal(
      await contents.executeJavaScript(
        'Object.keys(palisadeAssistNative).sort().join(",")',
      ),
      "request,subscribe",
    );
    assert.equal(
      await contents.executeJavaScript('typeof require + ":" + typeof process'),
      "undefined:undefined",
    );
  assert.equal(await contents.executeJavaScript('typeof selectedRoute'), 'undefined');
    assert.equal(
      await evaluate(
        `JSON.parse(await palisadeAssistNative.request(${message("status")})).error`,
      ),
      "unsupported-version",
    );
    async function hello() {
      assert.equal(
        await evaluate(
          "JSON.parse(await palisadeAssistNative.request(JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op:'hello',versions:[1]}))).ok",
        ),
        true,
      );
    }
    await hello();
    await contents.executeJavaScript(
      'new Promise(resolve=>{const frame=document.createElement("iframe");frame.id="child";frame.src="https://palisade.onyxops.org/frame";frame.onload=resolve;document.body.append(frame)})',
    );
    assert.equal(
      await contents.executeJavaScript(
        'typeof document.getElementById("child").contentWindow.palisadeAssistNative',
      ),
      "undefined",
    );
    phase = "full queue and independent stop";
    mode = "block";
    await contents.executeJavaScript(
      "window.pending=Array.from({length:8},()=>palisadeAssistNative.request(JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op:'enroll.prove',endpointId:'endpoint_fixture_01',challenge:'" +
        fixtures.requests.valid[2].challenge +
        "',account:'@fixture:palisade.test',device:'FIXTURE'})));void 0",
    );
    for (let i = 0; i < 100 && blocked.length < 8; i++) await pause(10);
    assert.equal(blocked.length, 8);
    assert.equal(
      await evaluate(
        `JSON.parse(await palisadeAssistNative.request(${message("stop")})).ok`,
      ),
      true,
    );
    assert(scopes.every((scope) => scope.signal.aborted));
    assert.equal(
      await contents.executeJavaScript(
        'Promise.all(pending).then(values=>values.every(text=>JSON.parse(text).error==="stopped"))',
      ),
      true,
    );
    for (const resolve of blocked.splice(0))
      resolve({ signature: fixtures.statements[0].signature });
    mode = "idle";
    await pause(30);
    await hello();
    phase = "owned lifetime and reload";
    await contents.executeJavaScript(
      "window.events=[];palisadeAssistNative.subscribe((...args)=>events.push(args));void 0",
    );
    assert.equal(
      await evaluate(
        "JSON.parse(await palisadeAssistNative.request(JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op:'capture.start',operation:'capture_fixture_01',room:'!fixture:palisade.test'}))).ok",
      ),
      true,
    );
    assert(
      savedScope.emit({
        event: "capture.status",
        operation: "capture_fixture_01",
        state: "live",
        reason: null,
      }),
    );
    await pause(30);
    assert.equal(
      await contents.executeJavaScript(
        'events.length===1 && events[0].length===1 && JSON.parse(events[0][0]).state==="live"',
      ),
      true,
    );
    await contents.reload();
    await new Promise((resolve) =>
      contents.isLoadingMainFrame()
        ? contents.once("did-finish-load", resolve)
        : resolve(),
    );
    assert(savedScope.signal.aborted);
    assert.equal(
      savedScope.emit({
        event: "capture.status",
        operation: "capture_fixture_01",
        state: "stopped",
        reason: "reload",
      }),
      false,
    );
    await hello();
    assert.equal(
      await evaluate(
        `JSON.parse(await palisadeAssistNative.request(${message("status")})).ok`,
      ),
      true,
    );
    phase = "external navigation";
    await contents.loadURL("https://other.example/native-fixture");
    assert.equal(
      await contents.executeJavaScript("typeof palisadeAssistNative"),
      "undefined",
    );
    assert(stops >= 2);
    fs.writeFileSync(
      path.join(__dirname, "../out/assist-native-bridge-result.json"),
      JSON.stringify({
        passed: true,
        electron: process.versions.electron,
        isolatedSurface: true,
        subframeDenied: true,
        safetyQueue: true,
        staleResultsRejected: true,
        nativeExecution: false,
      }),
    );
    bridge.dispose();
    win.destroy();
    clearTimeout(deadline);
    app.exit(0);
  })
  .catch((error) => {
    console.error(phase, error?.stack || String(error));
    bridge?.dispose();
    win?.destroy();
    clearTimeout(deadline);
    app.exit(1);
  });
