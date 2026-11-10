//! Product metadata is an identity check, never a substitute for signed installer hashes.
use super::stage::wide;
use std::{
    ffi::c_void,
    fs::File,
    io::{Read, Seek, SeekFrom},
    mem::size_of,
    path::Path,
    ptr,
};
use windows_sys::Win32::Storage::FileSystem::*;
type Result<T> = std::result::Result<T, &'static str>;

struct VersionInfo {
    bytes: Vec<u8>,
}
impl VersionInfo {
    fn read(path: &Path) -> Result<Self> {
        let name = wide(path);
        let size = unsafe { GetFileVersionInfoSizeW(name.as_ptr(), ptr::null_mut()) };
        if size == 0 || size > 1024 * 1024 {
            return Err("image-version-size");
        }
        let mut bytes = vec![0; size as usize];
        if unsafe { GetFileVersionInfoW(name.as_ptr(), 0, size, bytes.as_mut_ptr().cast()) } == 0 {
            return Err("image-version-read");
        }
        Ok(Self { bytes })
    }
    fn query(&self, name: &str, words: bool) -> Result<&[u8]> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut raw: *mut c_void = ptr::null_mut();
        let mut count = 0;
        if unsafe {
            VerQueryValueW(
                self.bytes.as_ptr().cast(),
                name.as_ptr(),
                &mut raw,
                &mut count,
            )
        } == 0
            || raw.is_null()
        {
            return Err("image-version-field");
        }
        let length = (count as usize)
            .checked_mul(if words { 2 } else { 1 })
            .ok_or("image-version-field")?;
        let start = (raw as usize)
            .checked_sub(self.bytes.as_ptr() as usize)
            .ok_or("image-version-field")?;
        self.bytes
            .get(start..start.checked_add(length).ok_or("image-version-field")?)
            .ok_or("image-version-field")
    }
    fn text(&self, field: &str) -> Result<String> {
        let translations = self.query("\\VarFileInfo\\Translation", false)?;
        if translations.is_empty() || translations.len() % 4 != 0 || translations.len() > 64 {
            return Err("image-version-locale");
        }
        for entry in translations.chunks_exact(4) {
            let language = u16::from_le_bytes([entry[0], entry[1]]);
            let encoding = u16::from_le_bytes([entry[2], entry[3]]);
            if let Ok(bytes) = self.query(
                &format!("\\StringFileInfo\\{language:04x}{encoding:04x}\\{field}"),
                true,
            ) {
                if bytes.len() > 1024 || bytes.len() % 2 != 0 {
                    return Err("image-version-field");
                }
                let mut words: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .collect();
                if words.pop() != Some(0) || words.contains(&0) {
                    return Err("image-version-field");
                }
                return String::from_utf16(&words).map_err(|_| "image-version-field");
            }
        }
        Err("image-version-field")
    }
}

pub fn validate(path: &Path, expected: &str) -> Result<()> {
    validate_kind(path, expected, false)
}

pub fn validate_installer(path: &Path, expected: &str) -> Result<()> {
    validate_kind(path, expected, true)
}

fn validate_kind(path: &Path, expected: &str, installer: bool) -> Result<()> {
    let version = crate::stable_version(expected)?;
    if version.iter().any(|value| *value > u16::MAX as u64) {
        return Err("image-version-range");
    }
    let mut file = File::open(path).map_err(|_| "image-read")?;
    let mut dos = [0; 64];
    file.read_exact(&mut dos).map_err(|_| "image-format")?;
    let offset = u32::from_le_bytes(dos[60..64].try_into().map_err(|_| "image-format")?) as u64;
    if &dos[..2] != b"MZ" || !(64..=1024 * 1024).contains(&offset) {
        return Err("image-format");
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| "image-format")?;
    let mut pe = [0; 26];
    file.read_exact(&mut pe).map_err(|_| "image-format")?;
    let machine = u16::from_le_bytes([pe[4], pe[5]]);
    let magic = u16::from_le_bytes([pe[24], pe[25]]);
    // NSIS uses a 32-bit bootstrap even for the signed Windows x64 application payload.
    if &pe[..4] != b"PE\0\0"
        || !((machine == 0x8664 && magic == 0x20b)
            || (installer && machine == 0x14c && magic == 0x10b))
    {
        return Err("image-architecture");
    }
    let info = VersionInfo::read(path)?;
    let fixed = info.query("\\", false)?;
    if fixed.len() < size_of::<VS_FIXEDFILEINFO>() {
        return Err("image-version-field");
    }
    // The resource buffer is byte-aligned; do not assume typed alignment from an untrusted PE.
    let fixed = unsafe { ptr::read_unaligned(fixed.as_ptr().cast::<VS_FIXEDFILEINFO>()) };
    let components = |high: u32, low: u32| {
        [
            (high >> 16) as u64,
            (high & 65535) as u64,
            (low >> 16) as u64,
            (low & 65535) as u64,
        ]
    };
    let expected = [version[0], version[1], version[2], 0];
    if fixed.dwSignature != 0xfeef04bd
        || components(fixed.dwProductVersionMS, fixed.dwProductVersionLS) != expected
        || components(fixed.dwFileVersionMS, fixed.dwFileVersionLS) != expected
        || info.text("ProductName")? != "Palisade"
        || info.text("CompanyName")? != "Onyx Operations Group"
    {
        return Err("image-identity");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_test_process_is_not_a_palisade_installer() {
        assert!(
            validate(
                &std::env::current_exe().unwrap(),
                &crate::packaged_version().unwrap()
            )
            .is_err()
        );
        assert!(validate(Path::new("C:\\missing-palisade-fixture.exe"), "0.3.4").is_err());
        assert!(validate(Path::new("C:\\missing-palisade-fixture.exe"), "65536.0.0").is_err());
    }
    #[test]
    fn windows_product_metadata_is_validated_on_a_nonexecuted_disposable_pe() {
        let directory = crate::windows::stage::tests::fixture();
        let source = directory.join("fixture.cs");
        let executable = directory.join("fixture.exe");
        std::fs::write(
            &source,
            r#"using System.Reflection;
[assembly: AssemblyProduct("Palisade")]
[assembly: AssemblyCompany("Onyx Operations Group")]
[assembly: AssemblyVersion("0.3.4.0")]
[assembly: AssemblyFileVersion("0.3.4.0")]
[assembly: AssemblyInformationalVersion("0.3.4")]
class Fixture { static void Main() {} }
"#,
        )
        .unwrap();
        let compiler = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
        let result = std::process::Command::new(compiler)
            .args(["/nologo", "/target:exe", "/platform:x64"])
            .arg(format!("/out:{}", executable.display()))
            .arg(source)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert!(validate(&executable, "0.3.4").is_ok());
        assert!(validate_installer(&executable, "0.3.4").is_ok());
        assert!(validate(&executable, "0.3.5").is_err());
        let mut bytes = std::fs::read(&executable).unwrap();
        let offset = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
        bytes[offset + 4..offset + 6].copy_from_slice(&0x14cu16.to_le_bytes());
        std::fs::write(&executable, bytes).unwrap();
        assert_eq!(validate(&executable, "0.3.4"), Err("image-architecture"));
        assert_eq!(
            validate_installer(&executable, "0.3.4"),
            Err("image-architecture")
        );
        let compiler = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
        let result = std::process::Command::new(compiler)
            .args(["/nologo", "/target:exe", "/platform:x86"])
            .arg(format!("/out:{}", executable.display()))
            .arg(directory.join("fixture.cs"))
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(validate(&executable, "0.3.4"), Err("image-architecture"));
        assert!(validate_installer(&executable, "0.3.4").is_ok());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
