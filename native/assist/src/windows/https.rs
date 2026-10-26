//! Endpoint-owned HTTPS. Fixed origin, OS certificate/revocation checks, no redirects or credentials.
use std::{
    ffi::c_void,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::{Win32::Networking::WinHttp::*, core::w};
const MAX_RESPONSE: usize = 128 * 1024;
struct Http(*mut c_void);
impl Drop for Http {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn request(path: &str, body: Option<&[u8]>) -> Result<Vec<u8>, &'static str> {
    if !matches!(
        path,
        "/api/v1/assist/issuer" | "/api/v1/assist/streams/endpoint-lease"
    ) || body.is_some_and(|b| b.len() > 4096)
    {
        return Err("invalid-request");
    }
    let start = Instant::now();
    unsafe {
        let session = Http(WinHttpOpen(
            w!("Palisade-Assist/2"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            ptr::null(),
            ptr::null(),
            0,
        ));
        if session.0.is_null() {
            return Err("network-unavailable");
        }
        if WinHttpSetTimeouts(session.0, 1000, 1000, 1000, 1000) == 0 {
            return Err("network-policy");
        }
        let connection = Http(WinHttpConnect(
            session.0,
            w!("palisade.onyxops.org"),
            443,
            0,
        ));
        if connection.0.is_null() {
            return Err("network-unavailable");
        }
        let path = wide(path);
        let method = if body.is_some() {
            w!("POST")
        } else {
            w!("GET")
        };
        let request = Http(WinHttpOpenRequest(
            connection.0,
            method,
            path.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err("network-unavailable");
        }
        let disabled: u32 =
            WINHTTP_DISABLE_REDIRECTS | WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_AUTHENTICATION;
        let revocation: u32 = WINHTTP_ENABLE_SSL_REVOCATION;
        if WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_DISABLE_FEATURE,
            (&disabled as *const u32).cast(),
            4,
        ) == 0
            || WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_ENABLE_FEATURE,
                (&revocation as *const u32).cast(),
                4,
            ) == 0
        {
            return Err("network-policy");
        }
        let data = body.unwrap_or_default();
        if WinHttpSendRequest(
            request.0,
            w!("Content-Type: application/json\r\nAccept: application/json\r\n"),
            u32::MAX,
            if data.is_empty() {
                ptr::null()
            } else {
                data.as_ptr().cast()
            },
            data.len() as u32,
            data.len() as u32,
            0,
        ) == 0
            || WinHttpReceiveResponse(request.0, ptr::null_mut()) == 0
        {
            return Err("network-failure");
        }
        let mut status = 0u32;
        let mut size = 4;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut size,
            ptr::null_mut(),
        ) == 0
            || status != 200
        {
            return Err("service-refused");
        }
        let mut result = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            if start.elapsed() >= Duration::from_secs(4) {
                return Err("network-timeout");
            }
            let mut count = 0;
            if WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut count,
            ) == 0
            {
                return Err("network-failure");
            }
            if count == 0 {
                break;
            }
            if result.len() + count as usize > MAX_RESPONSE {
                return Err("too-large");
            }
            result.extend_from_slice(&chunk[..count as usize]);
        }
        Ok(result)
    }
}
