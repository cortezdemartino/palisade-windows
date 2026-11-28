'use strict';
// Testing Center diagnostics contract v1 (project-palisade docs/contracts/tc/diagnostics-v1.json).
// The desktop app builds crash and error summaries itself and checks them with the same rules
// the Testing Center enforces, so nothing that would be refused is ever stored on the PC:
// enumerations, bounded numbers and narrow tokens only, file names without directories, no
// exception messages, no free text. The installation id is added later by the signed-in page.

const CRASH_KINDS = ['process-crash', 'oom', 'hang', 'unclean-exit', 'startup-failure', 'crash-loop', 'native-crash', 'panic'];
const ERROR_KINDS = ['exception', 'unhandled-rejection', 'render-failure', 'api-failure', 'sync-failure', 'auth-failure',
  'media-failure', 'assist-failure', 'ipc-failure', 'native-module-failure', 'helper-failure', 'update-failure',
  'install-failure', 'unrecoverable-state'];
const PROCESS_TYPES = ['browser', 'web-worker', 'main', 'renderer', 'gpu', 'utility', 'native-helper', 'updater', 'installer', 'native'];
const SUBSYSTEMS = ['shell', 'startup', 'auth', 'sync', 'crypto', 'messaging', 'voice', 'video', 'assist', 'updater', 'settings',
  'ipc', 'native', 'network', 'storage', 'ui', 'other'];
const REASONS = ['crashed', 'oom', 'killed', 'abnormal-exit', 'launch-failed', 'integrity-failure', 'memory-eviction',
  'unresponsive', 'timeout', 'unknown'];
const CATEGORIES = ['init', 'navigation', 'auth', 'sync', 'messaging', 'voice', 'assist', 'ipc', 'process', 'resource',
  'update', 'error', 'recovery'];
const OUTCOMES = ['start', 'success', 'failure', 'cancelled', 'timeout', 'info', 'warning'];

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const VERSION = /^\d{1,4}\.\d{1,5}\.\d{1,6}([-+][0-9A-Za-z.-]{1,32})?$/;
const BUILD_ID = /^[A-Za-z0-9._-]{1,64}$/;
const CHANNEL = /^[a-z][a-z0-9-]{1,23}$/;
const OS_VERSION = /^[0-9]{1,5}(\.[0-9]{1,6}){0,3}$/;
const ERROR_CODE = /^[A-Z][A-Z0-9_]{1,47}$/;
const EXCEPTION_TYPE = /^[A-Za-z][A-Za-z0-9_.]{0,63}$/;
const ACTION = /^[a-z][a-z0-9-]{1,47}(\.[a-z][a-z0-9-]{1,47}){0,2}$/;
const MODULE = /^[A-Za-z0-9_][A-Za-z0-9._@+-]{0,95}$/;
const FUNCTION = /^[A-Za-z0-9_$<>.[\] -]{1,128}$/;
const SENSITIVE = [/syt_[A-Za-z0-9_]{10,}/, /-----BEGIN [A-Z ]+-----/, /\bey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\./,
  /[A-Za-z0-9+/]{120,}={0,2}/, /[@!$+#][A-Za-z0-9._=\-/]{1,255}:[A-Za-z0-9.-]+\.[A-Za-z]{2,}/,
  /[^\s@]{1,64}@[A-Za-z0-9-]+\.[A-Za-z]{2,}/];

class DiagnosticError extends Error {
  constructor(code, field) { super(code + ' at ' + field); this.code = code; this.field = field; }
}
const plain = value => value !== null && typeof value === 'object' && !Array.isArray(value) &&
  [Object.prototype, null].includes(Object.getPrototypeOf(value));
function exact(value, required, optional, at) {
  if (!plain(value)) throw new DiagnosticError('P_TC_INVALID_REPORT', at);
  for (const key of Object.keys(value)) if (!required.includes(key) && !optional.includes(key)) throw new DiagnosticError('P_TC_INVALID_REPORT', at + '.' + key);
  for (const key of required) if (!(key in value)) throw new DiagnosticError('P_TC_INVALID_REPORT', at + '.' + key);
  return value;
}
function token(value, pattern, field, max = 128) {
  if (typeof value !== 'string' || value.length > max || !pattern.test(value)) throw new DiagnosticError('P_TC_INVALID_REPORT', field);
  if (SENSITIVE.some(p => p.test(value))) throw new DiagnosticError('P_TC_SENSITIVE_PAYLOAD', field);
  return value;
}
const optional = (value, check) => value === undefined || value === null ? undefined : check(value);
function oneOf(value, list, field) {
  if (!list.includes(value)) throw new DiagnosticError('P_TC_INVALID_REPORT', field);
  return value;
}
function int(value, min, max, field) {
  if (!Number.isInteger(value) || value < min || value > max) throw new DiagnosticError('P_TC_INVALID_REPORT', field);
  return value;
}

function validateBreadcrumb(value, at = 'breadcrumb') {
  const v = exact(value, ['at', 'id', 'category', 'subsystem', 'action', 'outcome'], ['code', 'correlationId'], at);
  const crumb = { at: int(v.at, 0, 8.64e12, at + '.at'), id: token(v.id, UUID, at + '.id', 36),
    category: oneOf(v.category, CATEGORIES, at + '.category'), subsystem: oneOf(v.subsystem, SUBSYSTEMS, at + '.subsystem'),
    action: token(v.action, ACTION, at + '.action', 100), outcome: oneOf(v.outcome, OUTCOMES, at + '.outcome') };
  if (v.code !== undefined) crumb.code = token(v.code, ERROR_CODE, at + '.code', 48);
  if (v.correlationId !== undefined) crumb.correlationId = token(v.correlationId, UUID, at + '.correlationId', 36);
  return crumb;
}

const REQUIRED = ['schemaVersion', 'reportId', 'kind', 'classification', 'occurredAt', 'client', 'sessionId', 'process', 'subsystem', 'fatal', 'frames'];
const OPTIONAL = ['installationId', 'correlationId', 'reason', 'exceptionType', 'errorCode', 'breadcrumbs', 'memoryMb', 'uptimeMs', 'crashLoopCount'];

/** Validates a report as the desktop app stores it (installationId is added when sent). */
function validateReport(value) {
  const v = exact(value, REQUIRED, OPTIONAL, 'report');
  if (v.schemaVersion !== 1) throw new DiagnosticError('P_TC_UNSUPPORTED_SCHEMA', 'schemaVersion');
  const c = exact(v.client, ['platform', 'appVersion', 'buildId', 'channel', 'os'], [], 'client');
  const os = exact(c.os, ['family'], ['version'], 'client.os');
  oneOf(c.platform, ['web', 'windows-electron', 'windows-native'], 'client.platform');
  token(c.appVersion, VERSION, 'client.appVersion', 64); token(c.buildId, BUILD_ID, 'client.buildId', 64);
  token(c.channel, CHANNEL, 'client.channel', 24);
  oneOf(os.family, ['windows', 'macos', 'linux', 'android', 'ios', 'chromeos', 'other'], 'client.os.family');
  optional(os.version, x => token(x, OS_VERSION, 'client.os.version', 32));
  token(v.reportId, UUID, 'reportId', 36); token(v.sessionId, UUID, 'sessionId', 36);
  optional(v.installationId, x => token(x, UUID, 'installationId', 36));
  optional(v.correlationId, x => token(x, UUID, 'correlationId', 36));
  const kind = oneOf(v.kind, [...CRASH_KINDS, ...ERROR_KINDS], 'kind');
  const classification = oneOf(v.classification, ['confirmed', 'inferred'], 'classification');
  if (classification === 'inferred' && kind !== 'unclean-exit' && kind !== 'crash-loop') throw new DiagnosticError('P_TC_INVALID_REPORT', 'classification');
  if (typeof v.occurredAt !== 'string' || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,3})?Z$/.test(v.occurredAt)) throw new DiagnosticError('P_TC_INVALID_REPORT', 'occurredAt');
  oneOf(v.process, PROCESS_TYPES, 'process'); oneOf(v.subsystem, SUBSYSTEMS, 'subsystem');
  optional(v.reason, x => oneOf(x, REASONS, 'reason'));
  optional(v.exceptionType, x => token(x, EXCEPTION_TYPE, 'exceptionType', 64));
  optional(v.errorCode, x => token(x, ERROR_CODE, 'errorCode', 48));
  if (typeof v.fatal !== 'boolean') throw new DiagnosticError('P_TC_INVALID_REPORT', 'fatal');
  if (!Array.isArray(v.frames) || v.frames.length > 64) throw new DiagnosticError('P_TC_INVALID_REPORT', 'frames');
  v.frames.forEach((f, i) => {
    const at = 'frames[' + i + ']';
    exact(f, ['module', 'inApp'], ['function', 'line', 'column'], at);
    token(f.module, MODULE, at + '.module', 96); optional(f.function, x => token(x, FUNCTION, at + '.function', 128));
    optional(f.line, x => int(x, 0, 1e7, at + '.line')); optional(f.column, x => int(x, 0, 1e6, at + '.column'));
    if (typeof f.inApp !== 'boolean') throw new DiagnosticError('P_TC_INVALID_REPORT', at + '.inApp');
  });
  if (v.breadcrumbs !== undefined) {
    if (!Array.isArray(v.breadcrumbs) || v.breadcrumbs.length > 50) throw new DiagnosticError('P_TC_INVALID_REPORT', 'breadcrumbs');
    v.breadcrumbs.forEach((b, i) => validateBreadcrumb(b, 'breadcrumbs[' + i + ']'));
  }
  optional(v.memoryMb, x => int(x, 0, 1048576, 'memoryMb')); optional(v.uptimeMs, x => int(x, 0, 31 * 86400000, 'uptimeMs'));
  optional(v.crashLoopCount, x => int(x, 0, 100, 'crashLoopCount'));
  return v;
}

const consentFor = kind => CRASH_KINDS.includes(kind) ? 'crashReports' : 'diagnostics';

/**
 * Stack frames as file name, function, line and column. Directories (which name the Windows
 * user), drive letters and asar paths are removed; frames outside the app are marked not in-app.
 */
function parseStack(stack, limit = 64) {
  if (typeof stack !== 'string') return [];
  const frames = [];
  for (const raw of stack.split('\n')) {
    const m = /^\s*at (?:(.+?) \()?(.+?):(\d+):(\d+)\)?$/.exec(raw);
    if (!m) continue;
    const location = m[2].replace(/^file:\/\/\/?/, '');
    const module = location.split(/[\\/]/).pop();
    if (!MODULE.test(module || '')) continue;
    const fn = m[1] && m[1].replace(/^async /, '').trim();
    const frame = { module, inApp: /app\.asar|[\\/]src[\\/]/.test(location) && !/node_modules|node:/.test(location),
      line: Number(m[3]), column: Number(m[4]) };
    if (fn && FUNCTION.test(fn)) frame.function = fn;
    frames.push(frame);
    if (frames.length >= limit) break;
  }
  return frames;
}

const exceptionType = error => error && typeof error.name === 'string' && EXCEPTION_TYPE.test(error.name) ? error.name : undefined;

module.exports = { validateReport, validateBreadcrumb, parseStack, exceptionType, consentFor, DiagnosticError,
  CRASH_KINDS, ERROR_KINDS, PROCESS_TYPES, SUBSYSTEMS, REASONS, CATEGORIES, OUTCOMES, ERROR_CODE, ACTION };
