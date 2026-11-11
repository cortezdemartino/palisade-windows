//! Fixed offline commands. This module never accepts an executable path, trust key or URL.
use super::{
    image,
    parent::{Parent, Scope},
    stage::{Directories, Handle, Stage, lock_regular, same_file, wide},
    update_lock::{InstallerPermit, UpdateLock, installing},
};
use crate::{MAX_ENVELOPE, VerifiedRelease};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    mem::{size_of, zeroed},
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{
        Com::*,
        Console::{GetStdHandle, STD_INPUT_HANDLE},
        Pipes::PeekNamedPipe,
        Threading::*,
    },
    UI::{
        Shell::*,
        WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW, SW_SHOWNORMAL},
    },
};
type Result<T> = std::result::Result<T, &'static str>;
pub const FILENAME: &str = "Palisade-Update.exe";
const PROTOCOL: &str = "palisade.update.helper";

fn report(value: Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{value}")
        .and_then(|_| stdout.flush())
        .map_err(|_| "parent-pipe-closed")
}
fn request_open() -> bool {
    let mut available = 0;
    unsafe {
        PeekNamedPipe(
            GetStdHandle(STD_INPUT_HANDLE),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut available,
            ptr::null_mut(),
        ) != 0
    }
}
fn nonce(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn commit_bytes(value: &str) -> Vec<u8> {
    format!("PALISADE-UPDATE-COMMIT-V1 {value}\n").into_bytes()
}
fn valid_commit(bytes: &[u8], value: &str) -> bool {
    nonce(value) && bytes == commit_bytes(value)
}
fn wait_commit(parent: &Parent, value: &str) -> Result<()> {
    if !nonce(value) {
        return Err("handoff-nonce");
    }
    let expected = commit_bytes(value);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut bytes = Vec::new();
    while bytes.len() < expected.len() {
        if Instant::now() >= deadline || !parent.alive() {
            return Err("handoff-canceled");
        }
        let mut available = 0;
        if unsafe {
            PeekNamedPipe(
                GetStdHandle(STD_INPUT_HANDLE),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut available,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err("handoff-canceled");
        }
        if available as usize > expected.len() - bytes.len() {
            return Err("handoff-format");
        }
        if available == 0 {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        let mut part = vec![0; available as usize];
        std::io::stdin()
            .read_exact(&mut part)
            .map_err(|_| "handoff-canceled")?;
        bytes.extend(part);
    }
    if !valid_commit(&bytes, value) {
        return Err("handoff-format");
    }
    Ok(())
}

fn own_image(expected: &Path) -> Result<File> {
    let own = std::env::current_exe().map_err(|_| "helper-image")?;
    let _own_directories = Directories::hold(own.parent().ok_or("helper-image")?)?;
    let _expected_directories = Directories::hold(expected.parent().ok_or("helper-image")?)?;
    let own_file = lock_regular(&own)?;
    let expected_file = lock_regular(expected)?;
    if !same_file(&own_file, &expected_file)? {
        return Err("helper-origin");
    }
    Ok(own_file)
}
fn bundled(parent: &Parent) -> PathBuf {
    parent.directory().join("resources/update").join(FILENAME)
}
fn overlap(stage: &Stage, parent: &Parent) -> Result<()> {
    if stage.overlaps_installation(parent.directory_guard())? {
        return Err("installation-staging-overlap");
    }
    Ok(())
}

struct Prepared {
    parent: Parent,
    stage: Stage,
    release: VerifiedRelease,
    installer: File,
    _manifest: File,
}
impl Prepared {
    fn open(parent: Parent, stage: Stage) -> Result<Self> {
        parent.scope()?;
        overlap(&stage, &parent)?;
        let mut manifest = stage.locked_file("release-v3.json")?;
        let mut bytes = Vec::new();
        (&mut manifest)
            .take(MAX_ENVELOPE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "manifest-read")?;
        let release = crate::verify_packaged(&bytes)?;
        let mut installer = stage.locked_file(&release.payload().filename)?;
        let deadline = Instant::now() + Duration::from_secs(600);
        crate::verify_installer_while(&mut installer, &release, || {
            parent.alive() && request_open() && Instant::now() < deadline
        })?;
        image::validate_installer(
            &stage.path().join(&release.payload().filename),
            &release.payload().version,
        )?;
        Ok(Self {
            parent,
            stage,
            release,
            installer,
            _manifest: manifest,
        })
    }
    fn receipt(&self, phase: &str, attempt: &str, error: Option<&str>) -> Result<()> {
        let name = match phase {
            "committed" | "installed" | "failed" if nonce(attempt) => {
                format!("result-{phase}-v1-{attempt}.json")
            }
            _ => return Err("receipt-phase"),
        };
        let mut file = self.stage.create_new(&name)?;
        let bytes = serde_json::to_vec(&json!({ "schema": 1, "nonce": attempt, "from": crate::packaged_version()?,
            "version": self.release.payload().version, "releaseSha256": self.release.digest()?, "phase": phase, "error": error })).map_err(|_| "receipt-format")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "receipt-write")
    }
    fn relaunch(&self, version: &str) -> Result<()> {
        let image_file = lock_regular(self.parent.image())?;
        image::validate(self.parent.image(), version)?;
        let resources = self.parent.directory().join("resources");
        let _resources = Directories::hold(&resources)?;
        let _archive = lock_regular(&self.parent.directory().join("resources/app.asar"))?;
        let process = shell_start(self.parent.image(), "", self.parent.directory(), false)?;
        drop(process);
        drop(image_file);
        Ok(())
    }
    fn retain_ambiguous_launch(&self, value: &str, _lock: &UpdateLock) -> ! {
        let _ = self.receipt("failed", value, Some("installer-ownership-unknown"));
        recovery_notice(true);
        // No process handle means no positive exit proof. Keep all ownership until OS shutdown.
        loop {
            std::thread::park_timeout(Duration::from_secs(60));
        }
    }
    fn apply(mut self, id: &str, value: &str, update_lock: UpdateLock) -> Result<()> {
        self.stage.mark_internet(&self.release)?;
        report(
            json!({ "state": "ready", "stage": id, "nonce": value, "version": self.release.payload().version,
            "releaseSha256": self.release.digest()? }),
        )?;
        wait_commit(&self.parent, value)?;
        self.receipt("committed", value, None)?;
        // Commit is irrevocable. EOF/cancel after this acknowledgement cannot imply installation stopped.
        report(json!({ "state": "committed", "stage": id, "nonce": value }))?;
        if let Err(code) = self.parent.wait_for_exit(120000) {
            let _ = self.receipt("failed", value, Some(code));
            return Err(code);
        }
        let result = (|| {
            crate::verify_installer(&mut self.installer, &self.release)?;
            let installer_path = self.stage.path().join(&self.release.payload().filename);
            image::validate_installer(&installer_path, &self.release.payload().version)?;
            let scope = self.parent.scope()?;
            let parameters = installer_parameters(scope, self.parent.directory(), value)?;
            let _permit = InstallerPermit::create(value)?;
            // The exact data file remains deny-write/delete locked until this owned child exits.
            let launched = shell_start(
                &installer_path,
                &parameters,
                self.stage.path(),
                scope == Scope::Machine,
            )?;
            let process = match launched {
                Some(handle) => Installer(handle),
                None => self.retain_ambiguous_launch(value, &update_lock),
            };
            if process.wait()? != 0 {
                return Err("installer-failed");
            }
            let installed = lock_regular(self.parent.image())?;
            image::validate(self.parent.image(), &self.release.payload().version)?;
            drop(installed);
            let _ = self.receipt("installed", value, None);
            // No installer remains. Release exclusion before the verified client starts.
            drop(update_lock);
            self.relaunch(&self.release.payload().version)
        })();
        if let Err(code) = result {
            let _ = self.receipt("failed", value, Some(code));
            // A failed installer can leave mixed-version resources. Never guess a rollback.
            recovery_notice(false);
            return Err(code);
        }
        Ok(())
    }
}

fn installer_parameters(scope: Scope, directory: &Path, attempt: &str) -> Result<String> {
    if !nonce(attempt) {
        return Err("handoff-nonce");
    }
    let directory = directory.to_str().ok_or("installation-path")?;
    if directory.chars().any(|c| c.is_control() || c == '"') {
        return Err("installation-path");
    }
    // NSIS requires unquoted /D as the final parameter. No command interpreter is used.
    Ok(format!(
        "/S {} --palisade-update={attempt} /D={directory}",
        if scope == Scope::Machine {
            "/allusers"
        } else {
            "/currentuser"
        }
    ))
}

fn shell_start(
    executable: &Path,
    parameters: &str,
    directory: &Path,
    elevated: bool,
) -> Result<Option<Handle>> {
    let initialized = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    if initialized < 0 {
        return Err("installer-shell");
    }
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com = Com;
    let file = wide(executable);
    let directory = wide(directory);
    let parameters: Vec<u16> = parameters.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = (if elevated { "runas" } else { "open" })
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut info: SHELLEXECUTEINFOW = unsafe { zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpFile = file.as_ptr();
    info.lpVerb = verb.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.lpDirectory = directory.as_ptr();
    info.nShow = SW_SHOWNORMAL;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(if unsafe { GetLastError() } == ERROR_CANCELLED {
            "approval-canceled"
        } else {
            "installer-start"
        });
    }
    if info.hProcess.is_null() || info.hProcess == INVALID_HANDLE_VALUE {
        return Ok(None);
    }
    Ok(Some(Handle(info.hProcess)))
}

fn recovery_notice(unknown_owner: bool) {
    let message = if unknown_owner {
        "Palisade cannot prove that its installer has finished. The updater is retaining its protection. Restart Windows before trying another update."
    } else {
        "Palisade could not finish updating or restarting. Your profile was not removed. Open the official Update Manager to inspect and repair the installation before trying again."
    };
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Palisade update needs attention"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn wait_positive(mut wait: impl FnMut() -> u32, mut retry: impl FnMut()) {
    while wait() != WAIT_OBJECT_0 {
        retry();
    }
}

struct Installer(Handle);
impl Installer {
    fn wait(&self) -> Result<u32> {
        wait_positive(
            || unsafe { WaitForSingleObject(self.0.0, INFINITE) },
            || std::thread::sleep(Duration::from_millis(250)),
        );
        let mut code = 0;
        if unsafe { GetExitCodeProcess(self.0.0, &mut code) } == 0 {
            return Err("installer-status");
        }
        Ok(code)
    }
}
impl Drop for Installer {
    fn drop(&mut self) {
        // Panic/ordinary error must not release verified file ownership while the installer lives.
        wait_positive(
            || unsafe { WaitForSingleObject(self.0.0, INFINITE) },
            || std::thread::sleep(Duration::from_millis(250)),
        );
    }
}

pub fn run(args: Vec<String>) -> Result<Option<Value>> {
    if args == ["--version"] {
        return Ok(Some(
            json!({ "protocol": PROTOCOL, "schema": 1, "version": crate::packaged_version()? }),
        ));
    }
    if args.len() == 2 && args[0] == "--status" {
        let parent =
            Parent::authenticate_status(args[1].parse::<u32>().map_err(|_| "parent-creator")?)?;
        let _own = own_image(&bundled(&parent))?;
        return Ok(Some(
            json!({ "protocol": PROTOCOL, "schema": 1, "state": if installing()? { "updating" } else { "idle" } }),
        ));
    }
    if args.len() < 3
        || !matches!(
            args[0].as_str(),
            "--stage" | "--inspect" | "--apply" | "--discard"
        )
        || !crate::stage_id(&args[2])
    {
        return Err("helper-arguments");
    }
    if (args[0] == "--apply" && (args.len() != 4 || !nonce(&args[3])))
        || (args[0] != "--apply" && args.len() != 3)
    {
        return Err("helper-arguments");
    }
    let parent = Parent::authenticate(args[1].parse::<u32>().map_err(|_| "parent-creator")?)?;
    let _cache_lock = UpdateLock::cache()?;
    // A surviving installer can retain this gate after its helper dies.
    if installing()? {
        return Err("update-busy");
    }
    let id = &args[2];
    if args[0] == "--stage" {
        let mut source = own_image(&bundled(&parent))?;
        let stage = Stage::create(id)?;
        overlap(&stage, &parent)?;
        let size = source.metadata().map_err(|_| "helper-image")?.len();
        if size == 0 || size > 32 * 1024 * 1024 {
            return Err("helper-image");
        }
        let mut destination = stage.create_new(FILENAME)?;
        source
            .seek(SeekFrom::Start(0))
            .map_err(|_| "helper-image")?;
        let copied = std::io::copy(&mut source.take(size + 1), &mut destination)
            .map_err(|_| "helper-copy")?;
        destination.sync_all().map_err(|_| "helper-copy")?;
        if copied != size || !parent.alive() || !request_open() {
            return Err("helper-copy");
        }
        return Ok(Some(
            json!({ "state": "created", "stage": id, "directory": stage.path() }),
        ));
    }
    if args[0] == "--discard" {
        let _own = own_image(&bundled(&parent))?;
        if let Some(stage) = Stage::open_existing(id)? {
            overlap(&stage, &parent)?;
            stage.discard()?;
        }
        return Ok(Some(json!({ "state": "discarded", "stage": id })));
    }
    let stage = Stage::open(id)?;
    let _own = own_image(&if args[0] == "--apply" {
        stage.path().join(FILENAME)
    } else {
        bundled(&parent)
    })?;
    let prepared = Prepared::open(parent, stage)?;
    if args[0] == "--inspect" {
        return Ok(Some(
            json!({ "state": "verified", "stage": id, "directory": prepared.stage.path(),
            "version": prepared.release.payload().version, "releaseSha256": prepared.release.digest()? }),
        ));
    }
    prepared.apply(id, &args[3], UpdateLock::install()?)?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_installer_parameters_preserve_scope_spaces_and_no_auto_run() {
        let path = Path::new("C:\\Users\\Fixture\\Programs\\Palisade App");
        let attempt = "a".repeat(64);
        assert_eq!(
            installer_parameters(Scope::User, path, &attempt).unwrap(),
            format!(
                "/S /currentuser --palisade-update={attempt} /D=C:\\Users\\Fixture\\Programs\\Palisade App"
            )
        );
        assert_eq!(
            installer_parameters(
                Scope::Machine,
                Path::new("C:\\Program Files\\Palisade"),
                &attempt
            )
            .unwrap(),
            format!("/S /allusers --palisade-update={attempt} /D=C:\\Program Files\\Palisade")
        );
        assert!(installer_parameters(Scope::User, Path::new("C:\\bad\"path"), &attempt).is_err());
    }
    #[test]
    fn failed_waits_never_settle_installer_ownership() {
        let sequence = [WAIT_FAILED, WAIT_TIMEOUT, WAIT_OBJECT_0];
        let mut index = 0;
        let mut retries = 0;
        wait_positive(
            || {
                let value = sequence[index];
                index += 1;
                value
            },
            || {
                retries += 1;
            },
        );
        assert_eq!(retries, 2);
        assert_eq!(index, 3);
    }
    #[test]
    fn argument_validation_cannot_select_paths_keys_or_commands() {
        for args in [
            vec![],
            vec!["--execute", "cmd.exe", "/c"],
            vec!["--apply", "1", "../x", "key"],
            vec![
                "--stage",
                "1",
                "77e14877-832f-44bc-9e92-b27eb6ba77ad",
                "extra",
            ],
        ] {
            assert!(run(args.into_iter().map(String::from).collect()).is_err());
        }
        assert!(!nonce("0"));
        assert!(!nonce(&"G".repeat(64)));
        assert!(nonce(&"a".repeat(64)));
        let nonce = "a".repeat(64);
        let line = commit_bytes(&nonce);
        assert!(valid_commit(&line, &nonce));
        assert!(!valid_commit(&line, &"b".repeat(64)));
        assert!(!valid_commit(
            &[line.as_slice(), b"second\n"].concat(),
            &nonce
        ));
    }
}
