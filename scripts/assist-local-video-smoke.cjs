"use strict";
const { app, BrowserWindow, ipcMain, desktopCapturer } = require("electron");
const path = require("node:path"),
  assert = require("node:assert/strict"),
  fs = require("node:fs");
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.resolve(".");
const { openGuardian } = require(path.join(root, "src/assist/guardian.cjs"));
const { createLocalVideoHop } = require(
  path.join(root, "src/assist/local-video-hop.cjs"),
);
const { createIsolatedCapture } = require(
  path.join(root, "src/assist/isolated-capture.cjs"),
);
const { createCapturePublication } = require(
  path.join(root, "src/assist/capture-publication.cjs"),
);
const { installNativeBridge } = require(
  path.join(root, "src/assist/native-bridge.cjs"),
);
const { chooseSource } = require(
  path.join(root, "src/assist/source-picker.cjs"),
);
const { installSessionPolicy } = require(
  path.join(root, "src/session-policy.cjs"),
);
app.disableHardwareAcceleration();
app.setPath(
  "userData",
  path.join(__dirname, "../out/assist-local-video-profile"),
);
const nativeMode = process.argv.includes("--native-publication");
const desktopMode = nativeMode || process.argv.includes("--owned-window");
let consumer, target, cover, hop, guardian, bridge;
const phases = [];
const timeout = setTimeout(() => {
  console.error("Local video fixture deadline", phases);
  hop?.dispose();
  guardian?.close();
  consumer?.destroy();
  target?.destroy();
  cover?.destroy();
  app.exit(1);
  // Allow the full 30s selection budget plus cold capture/renderer startup.
  // Liveness failures retain their much shorter component deadlines.
}, 70000);
ipcMain.on("palisade:local-video-event", (_event, message) => {
  phases.push(message.type + (message.stage ? ":" + message.stage : ""));
});
app
  .whenReady()
  .then(async () => {
    guardian = await openGuardian({
      binary: process.env.PALISADE_WINDOW_SMOKE_ASAR
        ? path.join(path.dirname(root), "assist/Palisade-Assist.exe")
        : path.resolve(".assist/Palisade-Assist.exe"),
      manifestFile: path.join(root, "src/assist/guardian-integrity.json"),
      probe: true,
    });
    consumer = new BrowserWindow({
      show: false,
      webPreferences: {
        partition: "assist-local-video-consumer",
        preload: path.join(
          root,
          nativeMode
            ? "src/assist/hosted-preload.cjs"
            : "src/assist/local-video-preload.cjs",
        ),
        additionalArguments: nativeMode ? ["--palisade-assist-native"] : [],
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        webSecurity: true,
        backgroundThrottling: false,
      },
    });
    const contents = consumer.webContents;
    contents.session.protocol.handle(
      "https",
      () =>
        new Response(
          "<!doctype html><title>Synthetic receiver</title><body></body>",
          {
            headers: {
              "Content-Type": "text/html",
              "Content-Security-Policy": "default-src 'none'",
            },
          },
        ),
    );
    installSessionPolicy(contents.session, () => contents);
    const sharedOptions = {
      consumer: contents,
      parent: consumer,
      enabled: true,
      inspect: (ports) => guardian.inspectLocalIce(ports),
      onState: (state) => phases.push(state),
    };
    if (desktopMode) {
      const captureOptions = {
        ...sharedOptions,
        getContents: () => contents,
        getWindow: () => consumer,
        enumerate: async () => {
          phases.push("enumerate-selected");
          const id = target.getMediaSourceId();
          return (
            await desktopCapturer.getSources({
              types: ["window"],
              thumbnailSize: { width: 0, height: 0 },
              fetchWindowIcons: false,
            })
          ).filter((source) => source.id === id);
        },
        choose: async (sources, options) => {
          phases.push("picker");
          const result = chooseSource(sources, options);
          let picker;
          for (let i = 0; i < 100; i++) {
            picker = BrowserWindow.getAllWindows().find(
              (window) =>
                window.webContents.getURL() ===
                "palisade-controls://app/assist-picker.html",
            );
            if (
              picker &&
              !picker.webContents.isLoadingMainFrame() &&
              (await picker.webContents.executeJavaScript(
                'document.querySelectorAll("input").length',
              ))
            )
              break;
            await new Promise((resolve) => setTimeout(resolve, 25));
          }
          assert(picker);
          await picker.webContents.executeJavaScript(
            'document.querySelector("input").click();document.getElementById("share").click();',
          );
          return result;
        },
      };
      if (nativeMode) {
        hop = createCapturePublication(captureOptions);
        bridge = installNativeBridge(contents, {
          enabled: true,
          dispatch: async (request, scope) => {
            if (request.op === "hello")
              return {
                version: 1,
                features: {
                  capture: true,
                  input: false,
                  clipboard: false,
                  files: false,
                },
              };
            if (request.op === "status")
              return {
                endpoint: { enrolled: false, endpointId: null, pin: "none" },
                capture: hop.status(),
                controller: { session: null, state: "idle" },
              };
            return hop.dispatch(request, scope);
          },
          stopAll: () => hop.stop(),
        });
      } else hop = createIsolatedCapture(captureOptions);
    } else hop = createLocalVideoHop(sharedOptions);
    await contents.loadURL("https://palisade.onyxops.org/local-video-fixture");
    let source = { kind: "synthetic" };
    if (desktopMode) {
      // A hidden test-process STARTUPINFO can suppress the first native show.
      consumer.show();
      consumer.hide();
      consumer.show();
      target = new BrowserWindow({
        width: 480,
        height: 300,
        x: 100,
        y: 100,
        show: false,
        frame: false,
        title: "Palisade owned capture fixture",
        webPreferences: {
          sandbox: true,
          contextIsolation: true,
          nodeIntegration: false,
          javascript: false,
          partition: "assist-owned-window-target",
        },
      });
      await target.loadURL(
        "data:text/html," +
          encodeURIComponent(
            '<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src &apos;none&apos;; style-src &apos;unsafe-inline&apos;"><style>html,body{margin:0;width:100%;height:100%;background:#20c080}</style>',
          ),
      );
      target.show();
      await new Promise((resolve) => setTimeout(resolve, 200));
      const id = target.getMediaSourceId();
      phases.push("enumerate-owned");
      const selected = (
        await desktopCapturer.getSources({
          types: ["window"],
          thumbnailSize: { width: 0, height: 0 },
          fetchWindowIcons: false,
        })
      ).find((value) => value.id === id);
      assert(selected);
      source = { kind: "desktop", id: selected.id };
      cover = new BrowserWindow({
        width: 100,
        height: 100,
        x: 290,
        y: 200,
        show: false,
        frame: false,
        alwaysOnTop: true,
        focusable: false,
        title: "Palisade fixture occluder",
        webPreferences: {
          sandbox: true,
          contextIsolation: true,
          nodeIntegration: false,
          javascript: false,
          partition: "assist-owned-window-cover",
        },
      });
      await cover.loadURL(
        "data:text/html," +
          encodeURIComponent(
            '<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src &apos;none&apos;; style-src &apos;unsafe-inline&apos;"><style>html,body{margin:0;width:100%;height:100%;background:#ef2040}</style>',
          ),
      );
      cover.showInactive();
      // The hosted renderer still cannot invoke legacy capture of even this fixture.
      assert.equal(
        await contents.executeJavaScript(
          `navigator.mediaDevices.getUserMedia({audio:false,video:{mandatory:{chromeMediaSource:'desktop',chromeMediaSourceId:${JSON.stringify(id)}}}}).then(stream=>{stream.getTracks().forEach(track=>track.stop());return 'granted';},error=>error.name)`,
        ),
        "NotAllowedError",
      );
    }
    phases.push("request");
    let value;
    if (nativeMode) {
      value = await contents.executeJavaScript(`(async()=>{
        const call=async(op,fields={})=>{const reply=JSON.parse(await palisadeAssistNative.request(JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op,...fields})));if(!reply.ok)throw Error(reply.error);return reply.result;};
        window.nativeEvents=[];palisadeAssistNative.subscribe(text=>nativeEvents.push(JSON.parse(text)));
        await call('hello',{versions:[1]});await call('capture.start',{operation:'capture_bridge_fixture',room:'!fixture:palisade.test'});
        const node=document.querySelector('video[data-palisade-assist-capture="capture_bridge_fixture"]');
        if(!node?.srcObject)throw Error('Missing canonical stream element');
        await call('capture.bind',{operation:'capture_bridge_fixture',trackSid:'TR_fixture01'});
        const repeated=JSON.parse(await palisadeAssistNative.request(JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op:'capture.bind',operation:'capture_bridge_fixture',trackSid:'TR_other01'})));
        if(repeated.error!=='stale-operation')throw Error('Repeated bind accepted');
        const state=await call('status');if(state.capture.trackSid!=='TR_fixture01')throw Error('Wrong bound SID');
        return {operation:'capture_bridge_fixture',elementId:node.id};
      })()`);
      assert.equal(hop.current().room, "!fixture:palisade.test");
      assert.equal(hop.current().trackSid, "TR_fixture01");
    } else value = desktopMode ? await hop.request() : await hop.start(source);
    if (desktopMode)
      assert.equal(
        (nativeMode ? hop.current().source : hop.selected()).id,
        target.getMediaSourceId(),
      );
    consumer.show();
    phases.push("pixel-check");
    const video = await contents.executeJavaScript(
      `(async()=>{const element=document.getElementById(${JSON.stringify(value.elementId)});const stream=element.srcObject;const probe=document.createElement('video');probe.srcObject=stream;probe.muted=true;document.body.append(probe);await probe.play();await Promise.race([new Promise(resolve=>probe.requestVideoFrameCallback(resolve)),new Promise((_resolve,reject)=>setTimeout(()=>reject(Error('No decoded fixture frame')),5000))]);const canvas=document.createElement('canvas');canvas.width=320;canvas.height=180;canvas.getContext('2d').drawImage(probe,0,0,320,180);const data=Array.from(canvas.getContext('2d').getImageData(160,90,1,1).data);probe.srcObject=null;probe.remove();return {tracks:stream.getVideoTracks().length,live:stream.getTracks()[0].readyState,pixel:data,node:typeof require,peer:typeof localVideoPeer};})()`,
    );
    assert.equal(video.tracks, 1);
    assert.equal(video.live, "live");
    assert.equal(video.node, "undefined");
    assert.equal(video.peer, "undefined");
    assert(video.pixel[1] > 140 && video.pixel[0] < 80 && video.pixel[2] > 70);
    if (desktopMode) {
      target.destroy();
      target = null;
      for (
        let i = 0;
        i < 100 && (nativeMode ? hop.current() !== null : hop.busy());
        i++
      )
        await new Promise((resolve) => setTimeout(resolve, 25));
      if (nativeMode) {
        assert.equal(
          hop.current(),
          null,
          "Closing the selected window must end the canonical publication",
        );
        assert.equal(
          await contents.executeJavaScript(
            'nativeEvents.some(event=>event.event==="capture.status"&&event.operation==="capture_bridge_fixture"&&event.state==="stopped")',
          ),
          true,
        );
      } else {
        assert.equal(
          hop.busy(),
          false,
          "Closing the selected window must stop capture",
        );
        assert.equal(hop.selected(), null);
      }
    } else hop.stop();
    for (let i = 0; i < 40; i++) {
      if (
        await contents.executeJavaScript(
          `document.getElementById(${JSON.stringify(value.elementId)})===null`,
        )
      )
        break;
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
    assert(
      await contents.executeJavaScript(
        `document.getElementById(${JSON.stringify(value.elementId)})===null`,
      ),
    );
    assert.equal(
      BrowserWindow.getAllWindows().filter(
        (window) => ![consumer, target, cover].includes(window),
      ).length,
      0,
    );
    fs.writeFileSync(
      path.join(
        __dirname,
        nativeMode
          ? "../out/assist-native-capture-result.json"
          : desktopMode
            ? "../out/assist-owned-window-result.json"
            : "../out/assist-local-video-result.json",
      ),
      JSON.stringify({
        passed: true,
        source: desktopMode
          ? "app-owned-window-with-owned-occluder"
          : "synthetic-canvas",
        peerCertificatesVerified: true,
        ownedUdpBindings: true,
        videoPixel: video.pixel,
        stop: true,
        electron: process.versions.electron,
        nativeBridge: nativeMode,
        liveKitPublication: "not-exercised",
      }),
    );
    console.log(
      (desktopMode ? "Owned window" : "Synthetic canvas") +
        " local DTLS video, certificate/route proof and stop passed.",
    );
    bridge?.dispose();
    hop.dispose();
    guardian.close();
    consumer.destroy();
    target?.destroy();
    cover?.destroy();
    clearTimeout(timeout);
    app.exit(0);
  })
  .catch((error) => {
    console.error("Local video fixture:", error.message, phases);
    hop?.dispose();
    guardian?.close();
    consumer?.destroy();
    target?.destroy();
    cover?.destroy();
    clearTimeout(timeout);
    app.exit(1);
  });
