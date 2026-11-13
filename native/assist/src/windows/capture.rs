//! Native capture provenance. The trusted application selects media; the hosted
//! page supplies neither geometry, HWND nor this non-deserializable binding.
use super::{
    input::Source,
    input_owner::InputOwner,
    process_input::custody::Client,
    source_stop::{SourceKey, SourceWitness},
};
use crate::{
    cancellation::SessionStop,
    capture::{Kind, Selection},
};
use std::{
    collections::BTreeSet,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::Arc,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{RemoteDesktop::ProcessIdToSessionId, Threading::*},
    UI::WindowsAndMessaging::*,
};

struct Pending {
    key: SourceKey,
    room: String,
    witness: Arc<SourceWitness>,
    _approval: InputOwner,
}
fn retire_pending(pending: &mut Option<Pending>, stop: &SessionStop) {
    if stop.is_stopped() || pending.as_ref().is_some_and(|value| !value.witness.live()) {
        *pending = None;
    }
}
enum NativeSource {
    Screen(Source),
    Window {
        hwnd: usize,
        pid: u32,
        _process: OwnedHandle,
    },
}
impl NativeSource {
    fn verify(selection: &Selection) -> Result<Self, &'static str> {
        super::input::ordinary_desktop()?;
        match selection.validate()? {
            Kind::Screen => {
                let source = selection.bounds.clone().ok_or("invalid-source")?;
                source.current_monitor()?;
                Ok(Self::Screen(source))
            }
            Kind::Window(value) => {
                let hwnd = usize::try_from(value).map_err(|_| "invalid-source")?;
                let mut pid = 0;
                if unsafe { IsWindow(hwnd as HWND) } == 0
                    || unsafe { IsWindowVisible(hwnd as HWND) } == 0
                    || unsafe { GetWindowThreadProcessId(hwnd as HWND, &mut pid) } == 0
                {
                    return Err("source-ended");
                }
                let process = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                        0,
                        pid,
                    )
                };
                if process.is_null() {
                    return Err("source-unavailable");
                }
                let process = unsafe { OwnedHandle::from_raw_handle(process) };
                let mut session = 0;
                let mut own_session = 0;
                if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0
                    || unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut own_session) } == 0
                    || session != own_session
                    || !super::parent::same_user_for_query(process.as_raw_handle(), false)
                {
                    return Err("source-unavailable");
                }
                let mut rect = RECT::default();
                let _dpi = super::input::DpiScope::enter()?;
                if unsafe { GetWindowRect(hwnd as HWND, &mut rect) } == 0
                    || rect.right <= rect.left
                    || rect.bottom <= rect.top
                {
                    return Err("source-unavailable");
                }
                Ok(Self::Window {
                    hwnd,
                    pid,
                    _process: process,
                })
            }
        }
    }
    fn live(&self) -> bool {
        if super::input::ordinary_desktop().is_err() {
            return false;
        }
        match self {
            Self::Screen(source) => source.current_monitor().is_ok(),
            Self::Window {
                hwnd,
                pid,
                _process,
            } => {
                let mut current = 0;
                unsafe {
                    WaitForSingleObject(_process.as_raw_handle(), 0) == WAIT_TIMEOUT
                        && IsWindow(*hwnd as HWND) != 0
                        && GetWindowThreadProcessId(*hwnd as HWND, &mut current) != 0
                        && current == *pid
                }
            }
        }
    }
}
struct Published {
    key: SourceKey,
    room: String,
    witness: Arc<SourceWitness>,
    source: Arc<NativeSource>,
    stream: Option<String>,
}
/// Only constructed after native source verification and tentative publication.
/// Service track-name corroboration remains mandatory before session admission.
#[derive(Clone)]
pub(crate) struct Binding {
    pub key: SourceKey,
    pub room: String,
    pub stream: String,
    pub geometry: Option<Source>,
    pub witness: Arc<SourceWitness>,
    source: Arc<NativeSource>,
}
impl Binding {
    pub(crate) fn live(&self) -> bool {
        self.witness.live() && self.source.live()
    }
}
impl crate::cancellation::StopWitness for Binding {
    fn stopped(&self) -> bool {
        !self.live()
    }
}
pub(crate) struct Capture {
    client: Arc<Client>,
    stop: SessionStop,
    pending: Option<Pending>,
    active: Option<Published>,
    retired: BTreeSet<String>,
}
impl Capture {
    pub(crate) fn new(client: Arc<Client>, stop: SessionStop) -> Self {
        Self {
            client,
            stop,
            pending: None,
            active: None,
            retired: BTreeSet::new(),
        }
    }
    pub(crate) fn harvest(&mut self) {
        retire_pending(&mut self.pending, &self.stop);
        if self.stop.is_stopped()
            || self
                .active
                .as_ref()
                .is_some_and(|value| !value.witness.live() || !value.source.live())
        {
            self.active = None;
        }
    }
    pub(crate) fn picking(&mut self) -> bool {
        self.harvest();
        self.pending.is_some()
    }
    pub(crate) fn begin(
        &mut self,
        document: u32,
        operation: &str,
        room: &str,
    ) -> Result<(), &'static str> {
        self.harvest();
        if self.stop.is_stopped() {
            return Err("revoked");
        }
        if !crate::grant::token(operation, 8, 128) || !crate::stream::room(room) {
            return Err("invalid-source");
        }
        if self.retired.contains(operation) || self.retired.len() >= 128 {
            return Err("stale-source");
        }
        if self.pending.is_some() || self.active.is_some() {
            return Err("source-busy");
        }
        self.retired.insert(operation.into());
        let key = SourceKey {
            document,
            operation: operation.into(),
        };
        let witness = self.client.source(&key)?;
        // No Palisade injector or cleanup holder can coexist with the picker.
        let approval = InputOwner::acquire()?;
        if self.stop.is_stopped() || !witness.live() {
            return Err("revoked");
        }
        self.pending = Some(Pending {
            key,
            room: room.into(),
            witness,
            _approval: approval,
        });
        Ok(())
    }
    pub(crate) fn selected(
        &mut self,
        document: u32,
        selection: Selection,
    ) -> Result<(), &'static str> {
        self.harvest();
        let pending = self.pending.as_ref().ok_or("stale-source")?;
        if pending.key.document != document || pending.key.operation != selection.operation {
            return Err("stale-source");
        }
        let source = Arc::new(NativeSource::verify(&selection)?);
        if self.stop.is_stopped() || !pending.witness.live() || !source.live() {
            return Err("source-ended");
        }
        let pending = self.pending.take().ok_or("stale-source")?;
        self.active = Some(Published {
            key: pending.key.clone(),
            room: pending.room.clone(),
            witness: pending.witness.clone(),
            source,
            stream: None,
        });
        // The picker has closed; no queued cleanup-generated gesture can approve
        // it later. Subsequent native permissions remain separately consented.
        drop(pending);
        Ok(())
    }
    pub(crate) fn bind(
        &mut self,
        document: u32,
        operation: &str,
        stream: &str,
    ) -> Result<(), &'static str> {
        self.harvest();
        let active = self.active.as_mut().ok_or("source-required")?;
        if active.key.document != document
            || active.key.operation != operation
            || active.stream.is_some()
            || !crate::stream::track(stream)
        {
            return Err("stale-source");
        }
        active.stream = Some(stream.into());
        Ok(())
    }
    pub(crate) fn binding(&mut self) -> Result<Binding, &'static str> {
        self.harvest();
        let active = self.active.as_ref().ok_or("source-required")?;
        Ok(Binding {
            key: active.key.clone(),
            room: active.room.clone(),
            stream: active.stream.clone().ok_or("source-unbound")?,
            geometry: match &*active.source {
                NativeSource::Screen(source) => Some(source.clone()),
                NativeSource::Window { .. } => None,
            },
            witness: active.witness.clone(),
            source: active.source.clone(),
        })
    }
    pub(crate) fn status(&mut self) -> serde_json::Value {
        self.harvest();
        if let Some(pending) = &self.pending {
            return serde_json::json!({"operation":pending.key.operation,"state":"picking","trackSid":null});
        }
        if let Some(active) = &self.active {
            return serde_json::json!({"operation":active.key.operation,"state":"live","trackSid":active.stream});
        }
        serde_json::json!({"operation":null,"state":"idle","trackSid":null})
    }
    pub(crate) fn stop(&mut self, document: u32, operation: &str) {
        if self
            .pending
            .as_ref()
            .is_some_and(|value| value.key.document == document && value.key.operation == operation)
        {
            self.pending = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|value| value.key.document == document && value.key.operation == operation)
        {
            self.active = None;
        }
    }
    pub(crate) fn end(&mut self) {
        self.pending = None;
        self.active = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_reap_retires_a_cancelled_picker_guard_without_a_page_poll_or_new_request() {
        let _serial = super::super::input_owner::APPROVAL_TEST_LOCK
            .lock()
            .unwrap();
        let event = super::super::process_tree::StopEvent::new().unwrap();
        let stop = SessionStop::from(Arc::new(std::sync::atomic::AtomicBool::new(false)));
        let mut pending = Some(Pending {
            key: SourceKey {
                document: 0,
                operation: "capture_fixture_01".into(),
            },
            room: "!fixture:palisade.test".into(),
            witness: SourceWitness::no_authority(&event),
            _approval: InputOwner::acquire().unwrap(),
        });
        assert!(InputOwner::acquire().is_err());
        retire_pending(&mut pending, &stop);
        assert!(pending.is_some());
        event.revoke().unwrap();
        // This is the same reap called by Endpoint.poll on every idle receive.
        retire_pending(&mut pending, &stop);
        assert!(pending.is_none());
        let _available = InputOwner::acquire().unwrap();
        assert!(!stop.is_stopped());
    }
}
