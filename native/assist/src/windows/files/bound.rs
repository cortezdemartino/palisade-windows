//! Publisher-side resource ownership. No path, role or authorizer is accepted
//! from an IPC caller. The companion contract must establish helper-side native
//! authority separately before that role is exposed.
use super::{FileStore, Metadata};
use crate::{
    stream::{SessionGate, StreamGrant},
    windows::{consent, input::ordinary_desktop, lease},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const PENDING: u8 = 0;
const SELECTING: u8 = 1;
const ACTIVE: u8 = 2;
const ENDED: u8 = 3;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    Send,
    Receive,
}
impl Direction {
    fn capability(self) -> &'static str {
        match self {
            Self::Send => "file_to_guest",
            Self::Receive => "file_to_host",
        }
    }
}
#[derive(Clone)]
struct Authority {
    gate: Arc<Mutex<SessionGate>>,
    stop: Arc<AtomicBool>,
    #[cfg(test)]
    clock: Option<Arc<AtomicU64>>,
    #[cfg(test)]
    file_confirmation: Option<bool>,
}
impl Authority {
    fn grant(&self, id: &str, capability: &str) -> Result<StreamGrant, &'static str> {
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        let mut gate = self.gate.lock().map_err(|_| "endpoint-failed")?;
        #[cfg(test)]
        let fixture = self.clock.as_ref();
        #[cfg(not(test))]
        let fixture: Option<&Arc<AtomicU64>> = None;
        if fixture.is_none() && ordinary_desktop().is_err() {
            self.stop.store(true, Ordering::SeqCst);
            gate.stop();
            return Err("protected-desktop");
        }
        // Read time after acquiring the gate so concurrent checks cannot create
        // a false wall-clock rollback by arriving in the opposite lock order.
        let now = match fixture {
            Some(clock) => clock.load(Ordering::SeqCst),
            None => lease::now()?,
        };
        let grant = gate.authorize(id, capability, now, Instant::now())?.clone();
        if self.stop.load(Ordering::SeqCst) {
            return Err("revoked");
        }
        Ok(grant)
    }
}
struct Operation {
    grant: StreamGrant,
    direction: Direction,
    transfer: String,
    metadata: Mutex<Option<Metadata>>,
    token: Mutex<Option<String>>,
    cancel: Arc<AtomicBool>,
    phase: AtomicU8,
    until: Mutex<Instant>,
}
struct State {
    authority: Authority,
    closed: Arc<AtomicBool>,
    failed: AtomicBool,
    operations: Mutex<BTreeMap<Direction, Arc<Operation>>>,
    // Opposite-direction dialogs/hashing cannot block each other. Both shards
    // share the same session-lifetime quota; cancellation does not refund it.
    stores: [Mutex<FileStore>; 2],
}
impl State {
    fn fail(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.failed.store(true, Ordering::SeqCst);
    }
    fn store(&self, direction: Direction) -> &Mutex<FileStore> {
        &self.stores[match direction {
            Direction::Send => 0,
            Direction::Receive => 1,
        }]
    }
    fn check(&self, op: &Operation) -> Result<(), &'static str> {
        let result = (|| {
            if self.closed.load(Ordering::SeqCst)
                || op.cancel.load(Ordering::SeqCst)
                || op.phase.load(Ordering::SeqCst) == ENDED
                || Instant::now() >= *op.until.lock().map_err(|_| "resource-failed")?
            {
                return Err("transfer-cancelled");
            }
            let current = self
                .authority
                .grant(&op.grant.grant_id, op.direction.capability())?;
            if current.canonical() != op.grant.canonical() {
                return Err("binding-mismatch");
            }
            if self.closed.load(Ordering::SeqCst)
                || op.cancel.load(Ordering::SeqCst)
                || Instant::now() >= *op.until.lock().map_err(|_| "resource-failed")?
            {
                return Err("transfer-cancelled");
            }
            Ok(())
        })();
        if result.is_err() {
            op.cancel.store(true, Ordering::SeqCst);
            if matches!(result, Err("resource-failed" | "endpoint-failed")) {
                self.fail();
            }
        }
        result
    }
    fn retire(&self, op: &Arc<Operation>) {
        {
            let mut operations = self.operations.lock().unwrap_or_else(|error| {
                self.fail();
                error.into_inner()
            });
            if operations
                .get(&op.direction)
                .is_some_and(|current| Arc::ptr_eq(current, op))
            {
                operations.remove(&op.direction);
            }
        }
    }
    fn sweep(&self) -> bool {
        let snapshot: Vec<_> = match self.operations.lock() {
            Ok(ops) => ops.values().cloned().collect(),
            Err(error) => {
                self.fail();
                error.into_inner().values().cloned().collect()
            }
        };
        for op in snapshot {
            if self.check(&op).is_ok() {
                continue;
            }
            // Flags have already invalidated authority, independently of a busy
            // store. The dialog hook/hash callback observes the same atomic flag.
            if op.phase.load(Ordering::SeqCst) == SELECTING {
                continue;
            }
            let mut store = match self.store(op.direction).try_lock() {
                Ok(store) => store,
                Err(std::sync::TryLockError::WouldBlock) => continue,
                Err(std::sync::TryLockError::Poisoned(error)) => {
                    self.fail();
                    error.into_inner()
                }
            };
            {
                let mut token = op.token.lock().unwrap_or_else(|error| {
                    self.fail();
                    error.into_inner()
                });
                if let Some(token) = token.take() {
                    store.cancel(&token);
                }
            }
            drop(store);
            self.retire(&op);
        }
        let mut busy = false;
        if self.closed.load(Ordering::SeqCst) {
            // Even a retired operation may still have a busy store. Ownership
            // is complete only after both shards have released every handle.
            for store in &self.stores {
                match store.try_lock() {
                    Ok(mut store) => store.transfers.clear(),
                    Err(std::sync::TryLockError::WouldBlock) => busy = true,
                    Err(std::sync::TryLockError::Poisoned(error)) => {
                        self.fail();
                        error.into_inner().transfers.clear();
                    }
                }
            }
        }
        !self
            .operations
            .lock()
            .unwrap_or_else(|error| {
                self.fail();
                error.into_inner()
            })
            .is_empty()
            || busy
    }
}
/// Native-owned request, never deserialized from the hosted page. Dropping a
/// request cancels its pending selection or active handle, not its successor.
pub struct FileRequest {
    owner: Weak<State>,
    operation: Arc<Operation>,
}
impl FileRequest {
    pub fn cancel(&self) {
        self.operation.cancel.store(true, Ordering::SeqCst);
    }
}
impl Drop for FileRequest {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[derive(Serialize)]
pub struct FileSelection {
    pub handle: String,
    pub metadata: Metadata,
}
pub struct SessionFiles {
    state: Arc<State>,
    watcher: crate::cleanup::RetainedWorker,
}
#[derive(Clone)]
pub struct FileRevoker(Arc<AtomicBool>);
impl FileRevoker {
    pub fn revoke(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
struct SelectionGuard<'a>(&'a Operation);
impl Drop for SelectionGuard<'_> {
    fn drop(&mut self) {
        if self.0.phase.load(Ordering::SeqCst) != ACTIVE {
            self.0.cancel.store(true, Ordering::SeqCst);
            self.0.phase.store(ENDED, Ordering::SeqCst);
        }
    }
}
impl SessionFiles {
    pub(crate) fn publisher(
        gate: Arc<Mutex<SessionGate>>,
        stop: Arc<AtomicBool>,
    ) -> Result<Self, &'static str> {
        if !cfg!(feature = "native-resources") {
            return Err("native-resources-disabled");
        }
        Self::create(Authority {
            gate,
            stop,
            #[cfg(test)]
            clock: None,
            #[cfg(test)]
            file_confirmation: None,
        })
    }
    fn create(authority: Authority) -> Result<Self, &'static str> {
        let closed = Arc::new(AtomicBool::new(false));
        let quota = Arc::new(AtomicU64::new(0));
        let state = Arc::new(State {
            authority,
            closed: closed.clone(),
            failed: AtomicBool::new(false),
            operations: Mutex::new(BTreeMap::new()),
            stores: [
                Mutex::new(FileStore::with_quota(closed.clone(), quota.clone())),
                Mutex::new(FileStore::with_quota(closed, quota)),
            ],
        });
        let watched = state.clone();
        let watcher = thread::Builder::new()
            .name("palisade-file-authority".into())
            .spawn(move || {
                loop {
                    if watched.authority.stop.load(Ordering::SeqCst) {
                        watched.closed.store(true, Ordering::SeqCst);
                    }
                    let active = watched.sweep();
                    if watched.closed.load(Ordering::SeqCst) && !active {
                        return if watched.failed.load(Ordering::SeqCst) {
                            Err("resource-cleanup-failed")
                        } else {
                            Ok(())
                        };
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            })
            .map_err(|_| "resource-watch-unavailable")?;
        Ok(Self {
            state,
            watcher: crate::cleanup::RetainedWorker::new(watcher),
        })
    }
    fn begin(
        &self,
        direction: Direction,
        grant: &str,
        transfer: &str,
        metadata: Option<Metadata>,
    ) -> Result<FileRequest, &'static str> {
        if self.state.closed.load(Ordering::SeqCst) || !crate::grant::token(transfer, 8, 128) {
            return Err("invalid-file-request");
        }
        if let Some(metadata) = &metadata {
            metadata.validate()?;
        }
        let grant = self.state.authority.grant(grant, direction.capability())?;
        let mut ops = self
            .state
            .operations
            .lock()
            .map_err(|_| "resource-failed")?;
        if self.state.closed.load(Ordering::SeqCst)
            || ops.contains_key(&direction)
            || ops.values().any(|op| op.transfer == transfer)
        {
            return Err("transfer-busy");
        }
        let operation = Arc::new(Operation {
            grant,
            direction,
            transfer: transfer.into(),
            metadata: Mutex::new(metadata),
            token: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            phase: AtomicU8::new(PENDING),
            until: Mutex::new(Instant::now() + Duration::from_secs(60)),
        });
        ops.insert(direction, operation.clone());
        Ok(FileRequest {
            owner: Arc::downgrade(&self.state),
            operation,
        })
    }
    pub fn request_send(&self, grant: &str, transfer: &str) -> Result<FileRequest, &'static str> {
        self.begin(Direction::Send, grant, transfer, None)
    }
    pub fn request_receive(
        &self,
        grant: &str,
        transfer: &str,
        metadata: Metadata,
    ) -> Result<FileRequest, &'static str> {
        self.begin(Direction::Receive, grant, transfer, Some(metadata))
    }
    fn owned<'a>(
        &self,
        request: &'a FileRequest,
        direction: Direction,
        token: Option<&str>,
    ) -> Result<&'a Arc<Operation>, &'static str> {
        if !Weak::ptr_eq(&request.owner, &Arc::downgrade(&self.state))
            || request.operation.direction != direction
        {
            return Err("binding-mismatch");
        }
        let op = &request.operation;
        if !self
            .state
            .operations
            .lock()
            .map_err(|_| "resource-failed")?
            .get(&direction)
            .is_some_and(|active| Arc::ptr_eq(active, op))
        {
            return Err("transfer-not-found");
        }
        if let Some(token) = token {
            if op.phase.load(Ordering::SeqCst) != ACTIVE
                || op.token.lock().map_err(|_| "resource-failed")?.as_deref() != Some(token)
            {
                return Err("binding-mismatch");
            }
        }
        self.state.check(op)?;
        Ok(op)
    }
    fn select_with(
        &self,
        request: &FileRequest,
        select: impl FnOnce(&mut FileStore, &Operation) -> Result<(String, Metadata), &'static str>,
    ) -> Result<FileSelection, &'static str> {
        let op = self.owned(request, request.operation.direction, None)?;
        op.phase
            .compare_exchange(PENDING, SELECTING, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "transfer-busy")?;
        let _guard = SelectionGuard(op);
        // Reservation and cancellation exist before either trusted native surface.
        if op.direction == Direction::Receive {
            let metadata = op
                .metadata
                .lock()
                .map_err(|_| "resource-failed")?
                .clone()
                .ok_or("invalid-file-offer")?;
            #[cfg(not(test))]
            let confirmed =
                consent::confirm_file(&op.grant, &op.transfer, &metadata, op.cancel.clone())?;
            #[cfg(test)]
            let confirmed = match self.state.authority.file_confirmation {
                Some(decision) => decision,
                None => {
                    consent::confirm_file(&op.grant, &op.transfer, &metadata, op.cancel.clone())?
                }
            };
            if !confirmed {
                return Err("file-offer-denied");
            }
        }
        self.state.check(op)?;
        let mut store = self
            .state
            .store(op.direction)
            .lock()
            .map_err(|_| "resource-failed")?;
        self.state.check(op)?;
        let (token, metadata) = select(&mut store, op)?;
        *op.token.lock().map_err(|_| "resource-failed")? = Some(token.clone());
        if let Err(error) = self.state.check(op) {
            store.cancel(&token);
            return Err(error);
        }
        let mut expected = op.metadata.lock().map_err(|_| "resource-failed")?;
        if expected
            .as_ref()
            .is_some_and(|expected| *expected != metadata)
        {
            store.cancel(&token);
            return Err("binding-mismatch");
        }
        *expected = Some(metadata.clone());
        *op.until.lock().map_err(|_| "resource-failed")? = Instant::now() + Duration::from_secs(30);
        op.phase.store(ACTIVE, Ordering::SeqCst);
        Ok(FileSelection {
            handle: token,
            metadata,
        })
    }
    pub fn select(&self, request: &FileRequest) -> Result<FileSelection, &'static str> {
        self.select_with(request, |store, op| match op.direction {
            Direction::Send => store.select_send(op.cancel.clone(), || self.state.check(op)),
            Direction::Receive => {
                let metadata = op
                    .metadata
                    .lock()
                    .map_err(|_| "resource-failed")?
                    .clone()
                    .ok_or("invalid-file-offer")?;
                let token = store
                    .select_receive(metadata.clone(), op.cancel.clone(), || self.state.check(op))?;
                Ok((token, metadata))
            }
        })
    }
    fn failed(&self, op: &Operation) {
        op.cancel.store(true, Ordering::SeqCst);
        op.phase.store(ENDED, Ordering::SeqCst);
    }
    pub fn read(
        &self,
        request: &FileRequest,
        token: &str,
        offset: u64,
    ) -> Result<Vec<u8>, &'static str> {
        let op = self.owned(request, Direction::Send, Some(token))?;
        let result = self
            .state
            .store(Direction::Send)
            .lock()
            .map_err(|_| "resource-failed")?
            .read(token, offset, || self.state.check(op));
        if result.is_err() {
            self.failed(op);
        } else {
            *op.until.lock().map_err(|_| "resource-failed")? =
                Instant::now() + Duration::from_secs(30);
        }
        result
    }
    pub fn write(
        &self,
        request: &FileRequest,
        token: &str,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, &'static str> {
        let op = self.owned(request, Direction::Receive, Some(token))?;
        let result = self
            .state
            .store(Direction::Receive)
            .lock()
            .map_err(|_| "resource-failed")?
            .write(token, offset, bytes, || self.state.check(op));
        if result.is_err() {
            self.failed(op);
        } else {
            *op.until.lock().map_err(|_| "resource-failed")? =
                Instant::now() + Duration::from_secs(30);
        }
        result
    }
    pub fn commit(&self, request: &FileRequest, token: &str) -> Result<Metadata, &'static str> {
        let op = self.owned(request, Direction::Receive, Some(token))?;
        let result = self
            .state
            .store(Direction::Receive)
            .lock()
            .map_err(|_| "resource-failed")?
            .commit(token, || self.state.check(op));
        self.failed(op);
        self.state.retire(op);
        result
    }
    pub fn close(&self) {
        self.revoker().revoke();
        if let Ok(ops) = self.state.operations.try_lock() {
            for op in ops.values() {
                op.cancel.store(true, Ordering::SeqCst);
            }
        }
        // The watcher owns sweeps/I/O. Urgent callers never take a blocking lock.
    }
    pub fn revoker(&self) -> FileRevoker {
        FileRevoker(self.state.closed.clone())
    }
    pub fn wait_cleanup(&self) -> Result<(), &'static str> {
        self.close();
        self.watcher.join()
    }
}
impl Drop for SessionFiles {
    fn drop(&mut self) {
        self.close();
        if self.watcher.is_finished() {
            let _ = self.watcher.join();
        }
    }
}
#[cfg(test)]
mod tests;
