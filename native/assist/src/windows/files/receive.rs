//! Receive verification and final publication through the existing owned handle.
use super::*;

impl Receive {
    pub(super) fn verify(
        &mut self,
        authorize: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        authorize()?;
        if self.written != self.metadata.size
            || self.file.metadata().map_err(|_| "destination-read")?.len() != self.metadata.size
        {
            return Err("integrity-failure");
        }
        // Never treat the incremental network hash as proof of the held file.
        // Re-read its actual contents before offering a final Save destination.
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| "destination-read")?;
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut bytes = [0u8; 65536];
        loop {
            authorize()?;
            let count = self.file.read(&mut bytes).map_err(|_| "destination-read")?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|&n| n <= self.metadata.size)
                .ok_or("integrity-failure")?;
            hash.update(&bytes[..count]);
        }
        if total != self.metadata.size || hex(&hash.clone().finalize()) != self.metadata.sha256 {
            return Err("integrity-failure");
        }
        self.hash = hash;
        self.file
            .seek(SeekFrom::End(0))
            .map_err(|_| "destination-read")?;
        self.file.sync_all().map_err(|_| "destination-write")?;
        authorize()?;
        self.verified = true;
        Ok(())
    }

    pub(super) fn save_to(
        &mut self,
        path: &Path,
        authorize: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Metadata, &'static str> {
        self.verify(authorize)?;
        // The staging name is random and its already-held parents cannot be
        // renamed. No caller can redirect this copy to an arbitrary source.
        if self
            .parent
            .destination(self.temporary.file_name().ok_or("invalid-file")?)?
            != self.temporary
        {
            return Err("staging-binding-mismatch");
        }
        let parent = ParentGuard::open(path)?;
        let name = path.file_name().ok_or("invalid-destination")?;
        Metadata {
            name: name.to_str().ok_or("unsupported-name")?.into(),
            ..self.metadata.clone()
        }
        .validate()?;
        if path.try_exists().map_err(|_| "destination-unavailable")? {
            return Err("destination-exists");
        }
        authorize()?;
        let token = identifier()?;
        let temporary = parent.destination(std::ffi::OsStr::new(&format!(
            ".palisade-save-{token}.partial"
        )))?;
        let file = private_file(&temporary, &parent)?;
        let mut destination = Receive {
            temporary,
            file,
            parent,
            metadata: self.metadata.clone(),
            name: name.into(),
            hash: Sha256::new(),
            written: 0,
            verified: false,
            destination: Some(path.into()),
            committed: false,
        };
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| "destination-read")?;
        let mut bytes = [0u8; 65536];
        loop {
            authorize()?;
            let count = self.file.read(&mut bytes).map_err(|_| "destination-read")?;
            if count == 0 {
                break;
            }
            if destination.written.saturating_add(count as u64) > destination.metadata.size {
                return Err("integrity-failure");
            }
            destination
                .file
                .write_all(&bytes[..count])
                .map_err(|_| "destination-write")?;
            destination.hash.update(&bytes[..count]);
            destination.written += count as u64;
        }
        // An adjacent destination temporary works across NTFS volumes without
        // a cross-volume rename or a visible, partially copied final file.
        destination.verify(authorize)?;
        destination.publish(authorize)?;
        Ok(self.metadata.clone())
    }

    fn publish(
        &mut self,
        authorize: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if !self.verified {
            return Err("integrity-required");
        }
        authorize()?;
        let mut ads = self.temporary.as_os_str().to_owned();
        ads.push(":Zone.Identifier");
        let mut zone = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(PathBuf::from(ads))
            .map_err(|_| "download-mark-failed")?;
        zone.write_all(b"[ZoneTransfer]\r\nZoneId=3\r\n")
            .and_then(|_| zone.sync_all())
            .map_err(|_| "download-mark-failed")?;
        drop(zone);
        authorize()?;
        let name: Vec<u16> = self
            .parent
            .destination(&self.name)?
            .as_os_str()
            .encode_wide()
            .collect();
        let size = size_of::<FILE_RENAME_INFO>() + name.len() * 2;
        let mut words = vec![0usize; size.div_ceil(size_of::<usize>())];
        // SAFETY: the aligned buffer holds the fixed structure plus the bounded
        // UTF-16 name. The held parents and existing private source handle are
        // native-owned; replacement is explicitly disabled.
        unsafe {
            let info = words.as_mut_ptr().cast::<FILE_RENAME_INFO>();
            (*info).Anonymous.ReplaceIfExists = false;
            (*info).RootDirectory = ptr::null_mut();
            (*info).FileNameLength = (name.len() * 2) as u32;
            ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
            if SetFileInformationByHandle(
                self.file.as_raw_handle(),
                FileRenameInfo,
                info.cast(),
                size as u32,
            ) == 0
            {
                return Err("destination-exists-or-unavailable");
            }
        }
        clear_delete_on_close(&self.file)?;
        self.committed = true;
        Ok(())
    }
}
