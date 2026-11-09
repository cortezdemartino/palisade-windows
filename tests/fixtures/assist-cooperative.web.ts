/**
 * Cooperative local input, version 1 (docs/51 §4.6): reference state machines shared with native
 * endpoints through `docs/assist-control/v2-yield/fixtures.json`.
 *
 * The host keeps their own mouse and keyboard during an approved session and always has
 * priority. Ordinary local activity makes the endpoint **yield** remote input. Yielding is
 * nonterminal and separate from `lifecycle suspended/stopped`, which (with Stop, revocation,
 * protected desktops, sensitive native UI, source/peer/lease loss, expiry, replacement and stream
 * end) still end input authority exactly as before. Yielding never extends the deadline, broadens
 * a capability, changes the controller or restores ended authority.
 *
 * - `createLocalYield` is the **endpoint** gate. Only the endpoint's own observation of local
 *   input, its cleanup and its authority decide yielding and readiness; nothing the helper sends
 *   resumes input. After readiness the endpoint drops every non-release input until the helper's
 *   `input-barrier` for that epoch arrives on the ordered control channel, so input queued or
 *   delayed from before the barrier can never be injected.
 * - `createYieldFollower` is the **helper**: it follows the endpoint's `input-state`, sends
 *   nothing but releases while yielded, clears its held-input bookkeeping (no automatic re-press)
 *   and answers readiness with the barrier.
 */
import {
  authorizes,
  ControlError,
  createOwnedInput,
  LOCAL_YIELD,
  type Capability,
  type ControlKind,
  type YieldReason,
} from './protocol';

type Held = { buttons: string[]; keys: string[] };
const none = (): Held => ({ buttons: [], keys: [] });

/** What the endpoint sends the helper (`input-state` body). */
export interface InputStateBody {
  state: 'yielded' | 'ready';
  epoch: number;
  reason: YieldReason | null;
}

/** What the endpoint must do after one step. */
export interface YieldOutput {
  /** For helper messages: whether the (press/move/text) input is injected now. */
  inject: boolean;
  /** `input-state` messages to send to the helper, in order. */
  send: InputStateBody[];
  /** Helper-owned buttons/keys to release now (injected ups). */
  release: Held;
  /** Helper-owned input the local person is also holding: forgotten, never released. */
  handedOver: Held;
  /** Input authority ends (report it as today: lifecycle suspended/stopped); null otherwise. */
  terminal: string | null;
}

/** The endpoint's own observation of the local person's input. */
export type LocalEvent =
  | { type: 'activity' }
  | { type: 'down' | 'up'; button: string }
  | { type: 'down' | 'up'; key: string };

/** What the endpoint knows about itself at that moment. */
export interface EndpointContext {
  /** Every release it injected has completed and nothing it owns is still pressed. */
  cleanupComplete: boolean;
  /**
   * The endpoint's own fingerprint of current authority: grants and generation, consent, lease
   * validity, peer and capture source. Any change while yielded ends input authority.
   */
  authority: string;
}

export type YieldGateState = 'negotiating' | 'legacy' | 'active' | 'yielded' | 'ready' | 'ended';

interface HelperMessage {
  session: string;
  kind: ControlKind;
  button?: unknown;
  code?: unknown;
  down?: unknown;
  epoch?: unknown;
}

function isRelease(m: HelperMessage): boolean {
  return (
    m.kind === 'release-all' ||
    ((m.kind === 'input-button' || m.kind === 'input-key') && m.down === false)
  );
}

/**
 * Endpoint gate for one helper peer. Call `helper` with a message that already parsed, verified
 * its grant and generation and passed replay and age checks; `local` with each local event;
 * `tick` periodically (and whenever cleanup or authority changes); `end` for every terminal
 * transition. Times are the endpoint's own monotonic milliseconds.
 */
export function createLocalYield(session: string, options: { idleMs?: number } = {}) {
  const idleMs = options.idleMs ?? LOCAL_YIELD.idleMs;
  const owned = createOwnedInput(session);
  let state: YieldGateState = 'negotiating';
  let epoch = 0;
  let reason: YieldReason | null = null;
  let snapshot = '';
  let lastLocal = Number.NEGATIVE_INFINITY;
  const localButtons = new Set<string>();
  const localKeys = new Set<string>();
  const out = (o: Partial<YieldOutput> = {}): YieldOutput => ({
    inject: false,
    send: [],
    release: none(),
    handedOver: none(),
    terminal: null,
    ...o,
  });
  const localHeld = () => ({ buttons: [...localButtons].sort(), keys: [...localKeys].sort() });
  const holding = () => localButtons.size > 0 || localKeys.size > 0;
  const busy = (at: number) => holding() || at - lastLocal < idleMs;
  const reasonAt = (at: number): YieldReason =>
    holding() ? 'local-held' : at - lastLocal < idleMs ? 'local-input' : 'neutralizing';

  function terminate(why: string): YieldOutput {
    const { released, handedOver } = owned.yieldTo(localHeld());
    state = 'ended';
    reason = null;
    return out({ release: released, handedOver, terminal: why });
  }

  /** Active (or ready) → yielded with a new epoch; owned input is let go, never the local's. */
  function startYield(at: number, ctx: EndpointContext): YieldOutput {
    if (epoch >= LOCAL_YIELD.maxEpoch) return terminate('epoch-exhausted');
    epoch += 1;
    state = 'yielded';
    snapshot = ctx.authority;
    reason = reasonAt(at);
    const { released, handedOver } = owned.yieldTo(localHeld());
    return out({ send: [{ state: 'yielded', epoch, reason }], release: released, handedOver });
  }

  /** Readiness check while yielded: all conditions, or an updated reason. */
  function settle(at: number, ctx: EndpointContext): YieldOutput {
    if (state !== 'yielded' && state !== 'ready') return out();
    if (ctx.authority !== snapshot) return terminate('authority-changed');
    if (state === 'ready') return out();
    if (!busy(at) && ctx.cleanupComplete) {
      state = 'ready';
      reason = null;
      return out({ send: [{ state: 'ready', epoch, reason: null }] });
    }
    const next = busy(at) ? reasonAt(at) : 'neutralizing';
    if (next === reason) return out();
    reason = next;
    return out({ send: [{ state: 'yielded', epoch, reason }] });
  }

  return {
    get state(): YieldGateState {
      return state;
    },
    get epoch(): number {
      return epoch;
    },
    get held(): Held {
      return owned.held;
    },

    helper(
      message: HelperMessage,
      grant: { capability: Capability; active: boolean },
      at: number,
      ctx: EndpointContext,
    ): YieldOutput {
      if (message.session !== session) throw new ControlError('binding-mismatch');
      if (!grant.active) throw new ControlError('revoked');
      if (!authorizes(message.kind, 'guest', grant.capability, 2)) {
        throw new ControlError('not-granted');
      }
      if (state === 'ended') throw new ControlError('revoked');
      if (message.kind === 'input-barrier') {
        const e = message.epoch as number;
        if (e === 0) {
          // Opt-in: must come before any input on this peer.
          if (state !== 'negotiating') throw new ControlError('stale-epoch');
          state = 'active';
          return busy(at) ? startYield(at, ctx) : out();
        }
        if (state === 'ready' && e === epoch) {
          state = 'active';
          return out();
        }
        throw new ControlError(state === 'yielded' && e === epoch ? 'yielded' : 'stale-epoch');
      }
      if (isRelease(message)) {
        // Releases are always applied; after a yield nothing is owned, so they inject nothing.
        return out({ release: owned.apply(message, grant) });
      }
      if (state === 'negotiating') state = 'legacy';
      if (state === 'yielded') throw new ControlError('yielded');
      // Ready but no barrier yet: this was sent before the helper saw readiness.
      if (state === 'ready') throw new ControlError('stale-epoch');
      owned.apply(message, grant);
      return out({ inject: true });
    },

    local(event: LocalEvent, at: number, ctx: EndpointContext): YieldOutput {
      if (state === 'ended') return out();
      lastLocal = at;
      if (event.type !== 'activity') {
        const set = 'button' in event ? localButtons : localKeys;
        const id = 'button' in event ? event.button : event.key;
        if (event.type === 'down') set.add(id);
        else set.delete(id);
      }
      // A peer without cooperative input keeps the old rule: local input ends remote input.
      if (state === 'legacy') return terminate('local-input');
      if (state === 'active' || state === 'ready') return startYield(at, ctx);
      if (state === 'yielded') return settle(at, ctx);
      return out();
    },

    tick(at: number, ctx: EndpointContext): YieldOutput {
      return settle(at, ctx);
    },

    /** Stop, revocation, lock, protected desktop, source/peer/lease loss, expiry, stream end. */
    end(why: string): YieldOutput {
      if (state === 'ended') return out();
      return terminate(why);
    },
  };
}

/** What the helper does with one `input-state` from the endpoint. */
export type FollowerAction =
  | { action: 'yield'; epoch: number; reason: YieldReason }
  | { action: 'update'; epoch: number; reason: YieldReason }
  | { action: 'barrier'; epoch: number }
  | { action: 'ignore'; error: 'stale-epoch' | 'replay' };

/**
 * Helper side for one native peer (opted in). `receive` takes an `input-state` that parsed and
 * whose session, grant and generation match this peer.
 */
export function createYieldFollower() {
  let state: 'active' | 'yielded' = 'active';
  let epoch = 0;
  let reason: YieldReason | null = null;
  let lastSeq = 0;
  return {
    get state(): 'active' | 'yielded' {
      return state;
    },
    get epoch(): number {
      return epoch;
    },
    get reason(): YieldReason | null {
      return reason;
    },
    receive(m: InputStateBody & { seq: number }): FollowerAction {
      if (m.seq <= lastSeq) return { action: 'ignore', error: 'replay' };
      lastSeq = m.seq;
      if (m.state === 'yielded') {
        const r = m.reason as YieldReason;
        if (m.epoch > epoch) {
          state = 'yielded';
          epoch = m.epoch;
          reason = r;
          return { action: 'yield', epoch, reason: r };
        }
        if (m.epoch === epoch && state === 'yielded') {
          reason = r;
          return { action: 'update', epoch, reason: r };
        }
        return { action: 'ignore', error: 'stale-epoch' };
      }
      if (state === 'yielded' && m.epoch === epoch) {
        state = 'active';
        reason = null;
        return { action: 'barrier', epoch };
      }
      return { action: 'ignore', error: 'stale-epoch' };
    },
    /** While yielded only releases go (they can only make things safer). */
    canSend(release: boolean): boolean {
      return state === 'active' || release;
    },
  };
}
