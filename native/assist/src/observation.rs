//! Bounded physical-host observation, not an input executor, grant registry or
//! consent source. All data is native-only and intentionally not deserializable.
use crate::cooperative::{FenceStamp, InputFence, LocalEvent, LocalObservation};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

pub const EVENT_CAPACITY: usize = 512;
const DEVICE_CAPACITY: usize = 32;
const HOLD_CAPACITY: usize = 256;
const RECONCILE_DEADLINE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Control {
    Key { scan: u16, extended: bool },
    Button(u8),
}
impl Control {
    fn valid(self) -> bool {
        match self {
            Self::Key { scan, .. } => (1..=255).contains(&scan),
            Self::Button(n) => n < 5,
        }
    }
    fn event(self, down: bool) -> LocalEvent {
        match self {
            Self::Button(n) if n < 5 => LocalEvent::Button {
                button: ["left", "right", "middle", "back", "forward"][usize::from(n)].into(),
                down,
            },
            Self::Key { scan, extended } => match key_name(scan, extended) {
                Some(code) => LocalEvent::Key { code, down },
                None => LocalEvent::Uncertain,
            },
            _ => LocalEvent::Uncertain,
        }
    }
}
// Invert the existing physical mapping off the hook thread, without translating
// keystrokes into characters or depending on the active keyboard layout.
fn key_name(scan: u16, extended: bool) -> Option<String> {
    let found = |code: &str| crate::input::scan_code2(code) == Some((scan, extended));
    for letter in b'A'..=b'Z' {
        let code = format!("Key{}", letter as char);
        if found(&code) {
            return Some(code);
        }
    }
    for n in 0..=9 {
        let code = format!("Digit{n}");
        if found(&code) {
            return Some(code);
        }
    }
    for n in 1..=24 {
        let code = format!("F{n}");
        if found(&code) {
            return Some(code);
        }
    }
    for code in [
        "ControlLeft",
        "ControlRight",
        "ShiftLeft",
        "ShiftRight",
        "AltLeft",
        "AltRight",
        "Enter",
        "Escape",
        "Tab",
        "Space",
        "Backspace",
        "Delete",
        "Insert",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Minus",
        "Equal",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "Semicolon",
        "Quote",
        "Backquote",
        "Comma",
        "Period",
        "Slash",
        "Numpad0",
        "Numpad1",
        "Numpad2",
        "Numpad3",
        "Numpad4",
        "Numpad5",
        "Numpad6",
        "Numpad7",
        "Numpad8",
        "Numpad9",
        "NumpadAdd",
        "NumpadSubtract",
        "NumpadMultiply",
        "NumpadDivide",
        "NumpadDecimal",
        "NumpadEnter",
        "NumLock",
        "CapsLock",
        "ScrollLock",
        "ContextMenu",
        "IntlBackslash",
        "IntlRo",
        "IntlYen",
        "KanaMode",
        "Lang1",
        "Lang2",
        "Convert",
        "NonConvert",
    ] {
        if found(code) {
            return Some(code.into());
        }
    }
    None
}
#[derive(Clone, Copy)]
pub enum NativeEvent {
    Activity,
    LowLevel {
        control: Control,
        down: bool,
    },
    Raw {
        device: usize,
        generation: u64,
        control: Control,
        down: bool,
    },
    Device {
        device: usize,
        generation: u64,
    },
    Removed {
        device: usize,
        generation: u64,
    },
    /// Includes injected events until a separate native intent arbiter proves
    /// their disposition. A matching application tag is never local consent.
    Uncertain,
}
pub struct StampedEvent {
    stamp: FenceStamp,
    event: NativeEvent,
    at: Instant,
}
impl StampedEvent {
    pub(crate) fn capture(
        fence: &Arc<InputFence>,
        event: NativeEvent,
        at: Instant,
    ) -> Result<Self, &'static str> {
        Ok(Self {
            stamp: fence.stamp()?,
            event,
            at,
        })
    }
}
struct Device {
    generation: u64,
    held: BTreeSet<Control>,
    removed: bool,
}
struct Transition {
    raw: bool,
    control: Control,
    down: bool,
    at: Instant,
}
#[derive(Clone, Copy)]
pub struct Snapshot {
    pub generation: u64,
    pub held: usize,
    pub pending: usize,
    pub uncertain: bool,
    /// An empty new table is never a qualified physical baseline.
    pub baseline_qualified: bool,
}
impl Snapshot {
    pub fn require_neutral(&self) -> Result<(), &'static str> {
        if !self.baseline_qualified {
            return Err("native-baseline-unqualified");
        }
        if self.uncertain || self.pending != 0 || self.held != 0 {
            return Err("local-input-unresolved");
        }
        Ok(())
    }
}
pub struct ObservationState {
    fence: Arc<InputFence>,
    generation: u64,
    last: Option<Instant>,
    devices: BTreeMap<usize, Device>,
    low_level: BTreeSet<Control>,
    reported: BTreeSet<Control>,
    pending: VecDeque<Transition>,
    uncertain: bool,
}
impl ObservationState {
    pub fn new(fence: Arc<InputFence>) -> Self {
        Self {
            fence,
            generation: 0,
            last: None,
            devices: BTreeMap::new(),
            low_level: BTreeSet::new(),
            reported: BTreeSet::new(),
            pending: VecDeque::with_capacity(EVENT_CAPACITY),
            uncertain: false,
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            generation: self.generation,
            held: self.reported.len(),
            pending: self.pending.len(),
            uncertain: self.uncertain,
            baseline_qualified: false,
        }
    }
    fn reconcile(&mut self, raw: bool, control: Control, down: bool, at: Instant) {
        if let Some(index) = self
            .pending
            .iter()
            .position(|p| p.raw != raw && p.control == control)
        {
            if self.pending[index].down == down {
                self.pending.remove(index);
            } else {
                self.uncertain = true;
            }
        } else if self.pending.len() == EVENT_CAPACITY {
            self.uncertain = true;
        } else {
            self.pending.push_back(Transition {
                raw,
                control,
                down,
                at,
            });
        }
    }
    fn transition(&mut self, event: NativeEvent, at: Instant) -> LocalEvent {
        let changed = match event {
            NativeEvent::Activity => return LocalEvent::Activity,
            NativeEvent::Uncertain => {
                self.uncertain = true;
                return LocalEvent::Uncertain;
            }
            NativeEvent::Device { device, generation } => {
                if device == 0
                    || generation == 0
                    || self.devices.len() == DEVICE_CAPACITY
                    || self.devices.contains_key(&device)
                {
                    self.uncertain = true;
                } else {
                    self.devices.insert(
                        device,
                        Device {
                            generation,
                            held: BTreeSet::new(),
                            removed: false,
                        },
                    );
                }
                // Arrival is descriptive. It cannot qualify a fresh device as
                // physically neutral or authorize input on it.
                return LocalEvent::Activity;
            }
            NativeEvent::Removed { device, generation } => {
                if let Some(d) = self
                    .devices
                    .get_mut(&device)
                    .filter(|d| d.generation == generation && !d.removed)
                {
                    d.removed = true;
                }
                self.uncertain = true;
                return LocalEvent::Uncertain;
            }
            NativeEvent::LowLevel { control, down } => {
                if !control.valid() {
                    self.uncertain = true;
                    return LocalEvent::Uncertain;
                }
                if down {
                    if self.low_level.len() == HOLD_CAPACITY && !self.low_level.contains(&control) {
                        self.uncertain = true;
                        return LocalEvent::Uncertain;
                    }
                    self.low_level.insert(control);
                } else {
                    self.low_level.remove(&control);
                }
                self.reconcile(false, control, down, at);
                control
            }
            NativeEvent::Raw {
                device,
                generation,
                control,
                down,
            } => {
                if !control.valid() {
                    self.uncertain = true;
                    return LocalEvent::Uncertain;
                }
                let Some(d) = self
                    .devices
                    .get_mut(&device)
                    .filter(|d| d.generation == generation && !d.removed)
                else {
                    self.uncertain = true;
                    return LocalEvent::Uncertain;
                };
                if down {
                    if d.held.len() == HOLD_CAPACITY && !d.held.contains(&control) {
                        self.uncertain = true;
                        return LocalEvent::Uncertain;
                    }
                    d.held.insert(control); // autorepeat never adds another hold
                } else if !d.held.remove(&control) {
                    self.uncertain = true;
                }
                self.reconcile(true, control, down, at);
                control
            }
        };
        if self.uncertain {
            return LocalEvent::Uncertain;
        }
        let held = self.low_level.contains(&changed)
            || self.devices.values().any(|d| d.held.contains(&changed))
            || self.pending.iter().any(|p| p.control == changed && p.down)
            || (self.reported.contains(&changed)
                && self.pending.iter().any(|p| p.control == changed));
        if held {
            self.reported.insert(changed);
        } else {
            self.reported.remove(&changed);
        }
        let event = changed.event(held);
        if matches!(event, LocalEvent::Uncertain) {
            self.uncertain = true;
        }
        event
    }
    pub fn consume(&mut self, entry: StampedEvent) -> Result<LocalObservation, &'static str> {
        // Queue identity/order belongs to native ingress, never a page clock.
        if !entry.stamp.is_for(&self.fence) {
            self.uncertain = true;
            self.fence.end();
            return Err("observation-binding");
        }
        if entry.stamp.generation() <= self.generation
            || self.last.is_some_and(|last| entry.at < last)
        {
            self.uncertain = true;
            self.fence.end();
            return Err("observation-replay");
        }
        if entry.stamp.generation()
            != self
                .generation
                .checked_add(1)
                .ok_or("observation-exhausted")?
        {
            self.uncertain = true;
        }
        self.generation = entry.stamp.generation();
        self.last = Some(entry.at);
        if self
            .pending
            .iter()
            .any(|p| entry.at.saturating_duration_since(p.at) >= RECONCILE_DEADLINE)
        {
            self.uncertain = true;
        }
        let event = self.transition(entry.event, entry.at);
        Ok(entry.stamp.attach(if self.uncertain {
            LocalEvent::Uncertain
        } else {
            event
        }))
    }
    /// Enqueue expiry on the SAME native ingress; never mint a consumer-side
    /// stamp ahead of callbacks that are already waiting in the queue.
    pub fn expired(&self, at: Instant) -> bool {
        !self.uncertain
            && self
                .pending
                .iter()
                .any(|p| at.saturating_duration_since(p.at) >= RECONCILE_DEADLINE)
    }
}

#[cfg(test)]
mod tests;
