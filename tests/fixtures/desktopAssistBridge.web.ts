/**
 * Palisade Assist cross-client bridge, web side (docs/45-ASSIST-CROSS-CLIENT-PROTOCOL.md).
 *
 * The Windows desktop app may expose one narrow, context-isolated object on the page:
 * `window.palisadeDesktopAssist`. It only *describes* what this device can do (which desktop
 * build, whether its screen-source picker exists, whether a native endpoint is installed). It is
 * never proof of authorization: the Palisade service decides who may join, view or share, and a
 * future native endpoint must verify its own independently issued authorization. Any page script
 * could forge this object, so every answer is validated strictly and only ever changes what the
 * interface *offers*, never what the service *allows*.
 *
 * Nothing secret crosses the bridge: no access token, device key, Matrix identity, room or
 * session identifier. Without a bridge, Palisade runs in ordinary browser mode.
 */

export const ASSIST_BRIDGE_PROTOCOL = 'palisade.assist.bridge';
/** Protocol versions this web build understands, newest first. */
export const ASSIST_BRIDGE_VERSIONS: readonly number[] = [1];
/** Longest wait for the desktop app to answer a capability request. */
export const ASSIST_BRIDGE_TIMEOUT_MS = 3_000;
/** Largest accepted capability report (serialized JSON characters). */
export const ASSIST_BRIDGE_MAX_REPORT = 2_048;
/** Desktop builds older than this cannot take part in Assist screen sharing. */
export const ASSIST_BRIDGE_MIN_DESKTOP = '0.3.0';

/** How this page is running. */
export type DesktopMode =
  /** No desktop bridge: an ordinary browser. */
  | 'browser'
  /** A compatible Windows desktop app answered with a valid report. */
  | 'desktop'
  /** A bridge answered, but its desktop build is below the supported minimum (update required). */
  | 'desktop-outdated'
  /** A bridge answered, but with no protocol version this build understands. */
  | 'desktop-incompatible'
  /** A bridge exists but did not answer validly in time (malformed, refused or timed out). */
  | 'desktop-error';

/**
 * Where a shared screen comes from.
 * - `browser`: the browser's own screen picker (getDisplayMedia).
 * - `desktop-picker`: the desktop app's own source picker, still reached through getDisplayMedia,
 *   so the same encrypted media path is used.
 * - `unavailable`: this device cannot share a screen in Palisade.
 */
export type CaptureSupport = 'browser' | 'desktop-picker' | 'unavailable';

export interface DesktopCapabilities {
  mode: DesktopMode;
  /** Desktop app version (`major.minor.patch`), when one validly reported it. */
  desktopVersion?: string;
  /** Negotiated protocol version, when a desktop bridge answered validly. */
  protocolVersion?: number;
  capture: CaptureSupport;
  /** Whether a native Assist endpoint is installed. Presence grants nothing (see docs/45). */
  nativeEndpoint: 'absent' | 'present';
  /** The desktop app can end Assist capture locally (tray/shortcut), independent of the page. */
  localRevocation: boolean;
  /** The desktop app offers an emergency stop that ends capture and any future input at once. */
  emergencyStop: boolean;
}

export const BROWSER_CAPABILITIES: DesktopCapabilities = Object.freeze({
  mode: 'browser',
  capture: 'browser',
  nativeEndpoint: 'absent',
  localRevocation: false,
  emergencyStop: false,
});

/** The object the desktop app exposes with Electron's contextBridge (docs/46). */
interface AssistBridge {
  protocol: unknown;
  getCapabilities: (request: CapabilityRequest) => unknown;
  onCapabilitiesChanged?: (listener: (report: unknown) => void) => unknown;
}

/** Exactly what the page sends: no identity, credential, room or session data. */
export interface CapabilityRequest {
  protocol: typeof ASSIST_BRIDGE_PROTOCOL;
  versions: number[];
}

const VERSION_PATTERN = /^\d{1,4}\.\d{1,4}\.\d{1,6}$/;
const REPORT_KEYS = [
  'protocol',
  'version',
  'desktopVersion',
  'capture',
  'nativeEndpoint',
  'localRevocation',
  'emergencyStop',
] as const;

function compareVersions(a: string, b: string): number {
  const left = a.split('.').map(Number);
  const right = b.split('.').map(Number);
  for (let i = 0; i < 3; i += 1) {
    const diff = (left[i] ?? 0) - (right[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * Validates one capability report. Anything not exactly a version-1 report — wrong protocol,
 * unknown or missing fields, wrong types, oversized, or a version this build does not know — is
 * refused. Never evaluates or follows anything in the report.
 */
export function parseCapabilityReport(report: unknown): DesktopCapabilities {
  const failed: DesktopCapabilities = {
    ...BROWSER_CAPABILITIES,
    mode: 'desktop-error',
    capture: 'unavailable',
  };
  if (typeof report !== 'object' || report === null || Array.isArray(report)) return failed;
  let size: number;
  try {
    size = JSON.stringify(report).length;
  } catch {
    return failed;
  }
  if (size > ASSIST_BRIDGE_MAX_REPORT) return failed;
  const record = report as Record<string, unknown>;
  if (
    Object.getPrototypeOf(record) !== Object.prototype &&
    Object.getPrototypeOf(record) !== null
  ) {
    return failed;
  }
  if (record.protocol !== ASSIST_BRIDGE_PROTOCOL) return failed;
  if (typeof record.version !== 'number' || !Number.isInteger(record.version)) return failed;
  if (!ASSIST_BRIDGE_VERSIONS.includes(record.version)) {
    return { ...failed, mode: 'desktop-incompatible' };
  }
  const keys = Object.keys(record);
  if (keys.length !== REPORT_KEYS.length || !REPORT_KEYS.every((key) => keys.includes(key))) {
    return failed;
  }
  const { desktopVersion, capture, nativeEndpoint, localRevocation, emergencyStop } = record;
  if (typeof desktopVersion !== 'string' || !VERSION_PATTERN.test(desktopVersion)) return failed;
  if (capture !== 'desktop-picker' && capture !== 'unavailable') return failed;
  if (nativeEndpoint !== 'absent' && nativeEndpoint !== 'present') return failed;
  if (typeof localRevocation !== 'boolean' || typeof emergencyStop !== 'boolean') return failed;
  if (compareVersions(desktopVersion, ASSIST_BRIDGE_MIN_DESKTOP) < 0) {
    return { ...failed, mode: 'desktop-outdated', desktopVersion };
  }
  return {
    mode: 'desktop',
    desktopVersion,
    protocolVersion: record.version,
    capture,
    nativeEndpoint,
    localRevocation,
    emergencyStop,
  };
}

function bridgeOf(scope: object): AssistBridge | undefined {
  const candidate = (scope as { palisadeDesktopAssist?: unknown }).palisadeDesktopAssist;
  if (typeof candidate !== 'object' || candidate === null) return undefined;
  const bridge = candidate as Partial<AssistBridge>;
  return typeof bridge.getCapabilities === 'function' ? (bridge as AssistBridge) : undefined;
}

export interface DesktopAssistAdapter {
  /** Last known capabilities (browser mode until a bridge answers validly). */
  current(): DesktopCapabilities;
  /** Asks the desktop app again. Late answers from earlier requests are ignored. */
  refresh(): Promise<DesktopCapabilities>;
  /** Called whenever the capabilities change. Returns the unsubscribe function. */
  subscribe(listener: () => void): () => void;
  /** Stops listening to the bridge; later answers and change events are ignored. */
  dispose(): void;
}

export function createDesktopAssistAdapter(
  scope: object = typeof window === 'undefined' ? {} : window,
  options: { timeoutMs?: number } = {},
): DesktopAssistAdapter {
  const timeoutMs = options.timeoutMs ?? ASSIST_BRIDGE_TIMEOUT_MS;
  const listeners = new Set<() => void>();
  let state: DesktopCapabilities = BROWSER_CAPABILITIES;
  let serial = 0;
  let disposed = false;
  let unsubscribeBridge: (() => void) | undefined;

  const set = (next: DesktopCapabilities) => {
    if (disposed) return;
    const changed = JSON.stringify(next) !== JSON.stringify(state);
    state = next;
    if (changed) for (const listener of listeners) listener();
  };

  const watch = (bridge: AssistBridge) => {
    if (unsubscribeBridge || typeof bridge.onCapabilitiesChanged !== 'function') return;
    try {
      const off = bridge.onCapabilitiesChanged((report) => {
        if (disposed) return;
        // A change event invalidates any request still waiting for an older answer.
        serial += 1;
        set(parseCapabilityReport(report));
      });
      unsubscribeBridge =
        typeof off === 'function' ? () => void (off as () => unknown)() : undefined;
    } catch {
      set(parseCapabilityReport(undefined));
    }
  };

  const refresh = async (): Promise<DesktopCapabilities> => {
    if (disposed) return state;
    const request = ++serial;
    const bridge = bridgeOf(scope);
    // No bridge: an ordinary browser. A desktop build without the bridge cannot be told apart
    // from a browser (its page exposes nothing); its screen capture then fails visibly.
    if (!bridge) {
      set(BROWSER_CAPABILITIES);
      return state;
    }
    if (bridge.protocol !== ASSIST_BRIDGE_PROTOCOL) {
      set(parseCapabilityReport(undefined));
      return state;
    }
    watch(bridge);
    const message: CapabilityRequest = {
      protocol: ASSIST_BRIDGE_PROTOCOL,
      versions: [...ASSIST_BRIDGE_VERSIONS],
    };
    let timer: ReturnType<typeof setTimeout> | undefined;
    const answer = await Promise.race([
      Promise.resolve()
        .then(() => bridge.getCapabilities(message))
        .then(
          (report) => parseCapabilityReport(report),
          () => parseCapabilityReport(undefined),
        ),
      new Promise<DesktopCapabilities>((resolve) => {
        timer = setTimeout(() => {
          resolve(parseCapabilityReport(undefined));
        }, timeoutMs);
      }),
    ]);
    clearTimeout(timer);
    // Only the newest request may update the state (no stale answer after a change or dispose).
    if (request === serial) set(answer);
    return state;
  };

  return {
    current: () => state,
    refresh,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose() {
      disposed = true;
      listeners.clear();
      unsubscribeBridge?.();
      unsubscribeBridge = undefined;
    },
  };
}
