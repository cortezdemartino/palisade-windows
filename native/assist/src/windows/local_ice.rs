//! Read-only ownership proof for a capture hop that must stay on this device.
//! Root identity is the already validated creator, never a renderer-supplied PID.
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    mem::{size_of, zeroed},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    NetworkManagement::IpHelper::*,
    Networking::WinSock::AF_INET,
    System::{Diagnostics::ToolHelp::*, RemoteDesktop::ProcessIdToSessionId, Threading::*},
};
#[derive(Serialize)]
pub struct LocalCandidate {
    pub port: u16,
    pub address: String,
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn process_tree() -> Result<BTreeMap<u32, u32>, &'static str> {
    unsafe {
        let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0));
        if snapshot.0 == INVALID_HANDLE_VALUE {
            return Err("ice-process-query");
        }
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut result = BTreeMap::new();
        let mut more = Process32FirstW(snapshot.0, &mut entry);
        while more != 0 {
            if result.len() >= 65536 {
                return Err("ice-process-query");
            }
            result.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            more = Process32NextW(snapshot.0, &mut entry);
        }
        Ok(result)
    }
}
fn descendant(tree: &BTreeMap<u32, u32>, mut pid: u32, root: u32) -> bool {
    for _ in 0..16 {
        if pid == root {
            return true;
        }
        let Some(parent) = tree.get(&pid) else {
            return false;
        };
        if *parent == pid || *parent == 0 {
            return false;
        }
        pid = *parent;
    }
    false
}
fn prove_lineage(
    tree: &BTreeMap<u32, u32>,
    mut pid: u32,
    root: u32,
    probe: bool,
) -> Result<Vec<Handle>, &'static str> {
    let mut handles = Vec::new();
    let mut child_time = None;
    let mut expected_session = None;
    for _ in 0..16 {
        unsafe {
            let process = Handle(OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            ));
            if process.0.is_null()
                || WaitForSingleObject(process.0, 0) != WAIT_TIMEOUT
                || !super::parent::same_user_for_query(process.0, probe)
            {
                return Err("ice-port-unowned");
            }
            let created = super::parent::creation_time(process.0)?;
            let mut session = 0;
            if child_time.is_some_and(|child| created > child)
                || ProcessIdToSessionId(pid, &mut session) == 0
                || session == 0
                || expected_session.is_some_and(|value| value != session)
            {
                return Err("ice-port-unowned");
            }
            child_time = Some(created);
            expected_session = Some(session);
            handles.push(process);
            if pid == root {
                return Ok(handles);
            }
            pid = *tree.get(&pid).ok_or("ice-port-unowned")?;
        }
    }
    Err("ice-port-unowned")
}
fn rows() -> Result<Vec<MIB_UDPROW_OWNER_PID>, &'static str> {
    unsafe {
        let mut bytes = 0u32;
        let result = GetExtendedUdpTable(
            ptr::null_mut(),
            &mut bytes,
            0,
            AF_INET as u32,
            UDP_TABLE_OWNER_PID,
            0,
        );
        if result != ERROR_INSUFFICIENT_BUFFER || !(4..=2 * 1024 * 1024).contains(&bytes) {
            return Err("ice-socket-query");
        }
        let mut storage = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        if GetExtendedUdpTable(
            storage.as_mut_ptr().cast(),
            &mut bytes,
            0,
            AF_INET as u32,
            UDP_TABLE_OWNER_PID,
            0,
        ) != NO_ERROR
        {
            return Err("ice-socket-query");
        }
        let table = storage.as_ptr().cast::<MIB_UDPTABLE_OWNER_PID>();
        let count = (*table).dwNumEntries as usize;
        if count > 16384
            || std::mem::offset_of!(MIB_UDPTABLE_OWNER_PID, table)
                .checked_add(count * size_of::<MIB_UDPROW_OWNER_PID>())
                .is_none_or(|n| n > bytes as usize)
        {
            return Err("ice-socket-query");
        }
        Ok(std::slice::from_raw_parts((*table).table.as_ptr(), count).to_vec())
    }
}
pub fn inspect(root: u32, ports: &[u16], probe: bool) -> Result<Vec<LocalCandidate>, &'static str> {
    if root == 0
        || ports.is_empty()
        || ports.len() > 64
        || ports.contains(&0)
        || ports.iter().copied().collect::<BTreeSet<_>>().len() != ports.len()
    {
        return Err("invalid-ice-query");
    }
    let tree = process_tree()?;
    let table = rows()?;
    let mut result = Vec::new();
    let mut retained = Vec::new();
    for &port in ports {
        let matches: Vec<_> = table
            .iter()
            .filter(|row| u16::from_be(row.dwLocalPort as u16) == port)
            .collect();
        // Refuse reused/ambiguous UDP ports, including another application.
        if matches.len() != 1 {
            return Err("ice-port-unowned");
        }
        let row = matches[0];
        if !descendant(&tree, row.dwOwningPid, root) {
            return Err("ice-port-unowned");
        }
        retained.extend(prove_lineage(&tree, row.dwOwningPid, root, probe)?);
        unsafe {
            let process = Handle(OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                row.dwOwningPid,
            ));
            if process.0.is_null() {
                return Err("ice-port-unowned");
            }
            let (mut owner_session, mut root_session) = (0, 0);
            if !super::parent::same_user_for_query(process.0, probe)
                || ProcessIdToSessionId(row.dwOwningPid, &mut owner_session) == 0
                || ProcessIdToSessionId(root, &mut root_session) == 0
                || owner_session == 0
                || owner_session != root_session
            {
                return Err("ice-port-unowned");
            }
        }
        let octets = row.dwLocalAddr.to_ne_bytes();
        let address = if octets == [0, 0, 0, 0] {
            "127.0.0.1".into()
        } else {
            std::net::Ipv4Addr::from(octets).to_string()
        };
        if octets[0] >= 224 {
            return Err("ice-port-unowned");
        }
        result.push(LocalCandidate { port, address });
    }
    // A socket disappearing or changing owner during identity checks is refused.
    let again = rows()?;
    for &port in ports {
        let old = table
            .iter()
            .find(|row| u16::from_be(row.dwLocalPort as u16) == port)
            .ok_or("ice-port-unowned")?;
        if again
            .iter()
            .filter(|row| u16::from_be(row.dwLocalPort as u16) == port)
            .count()
            != 1
        {
            return Err("ice-port-unowned");
        }
        let matching = again
            .iter()
            .filter(|row| {
                u16::from_be(row.dwLocalPort as u16) == port
                    && row.dwOwningPid == old.dwOwningPid
                    && row.dwLocalAddr == old.dwLocalAddr
            })
            .count();
        if matching != 1 {
            return Err("ice-port-unowned");
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ancestry_is_bounded_and_cannot_cross_or_cycle() {
        let tree = BTreeMap::from([(10, 1), (11, 10), (12, 11), (20, 1), (21, 21)]);
        assert!(descendant(&tree, 12, 10));
        assert!(!descendant(&tree, 20, 10));
        assert!(!descendant(&tree, 21, 10));
    }
}
