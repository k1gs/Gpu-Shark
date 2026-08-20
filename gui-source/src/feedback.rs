use gpu_shark::SysInfo;
use serde::Deserialize;
use serde_json::json;
use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;
use windows_sys::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
};

const HOST: &str = env!(
    "GPU_SHARK_FEEDBACK_HOST",
    "GPU_SHARK_FEEDBACK_HOST must be supplied by the public release build"
);
const PATH: &str = env!(
    "GPU_SHARK_FEEDBACK_PATH",
    "GPU_SHARK_FEEDBACK_PATH must be supplied by the public release build"
);
const MAX_PAYLOAD: usize = 256 * 1024;

#[derive(Debug)]
pub enum SubmitError {
    InvalidPayload(String),
    PayloadTooLarge,
    RateLimited,
    Server,
    Network(String),
}

#[derive(Deserialize)]
struct AcceptedResponse {
    report_id: String,
    status: String,
}

struct InternetHandle(*mut c_void);

impl Drop for InternetHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn clipped(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn save_rejected_payload(body: &[u8], response: &[u8]) -> String {
    let Some(root) = std::env::var_os("LOCALAPPDATA") else {
        return "server returned invalid_payload".into();
    };
    let folder = PathBuf::from(root).join("GPU Shark").join("reports");
    if fs::create_dir_all(&folder).is_err() {
        return "server returned invalid_payload".into();
    }
    let payload_path = folder.join("last-rejected-feedback.json");
    let response_path = folder.join("last-feedback-response.txt");
    let _ = fs::write(&payload_path, body);
    let _ = fs::write(&response_path, response);
    format!("invalid_payload; saved request: {}", payload_path.display())
}

pub fn submit(
    info: Option<&SysInfo>,
    message: &str,
    contact: Option<&str>,
    locale: &str,
    consent: bool,
) -> Result<String, SubmitError> {
    if !consent || message.trim().is_empty() {
        return Err(SubmitError::InvalidPayload(
            "message or consent is missing".into(),
        ));
    }
    let contact = contact.map(str::trim).filter(|value| !value.is_empty());
    if contact.is_some_and(|value| value.len() > 254 || !value.contains('@')) {
        return Err(SubmitError::InvalidPayload(
            "contact must be a valid email or empty".into(),
        ));
    }

    // SysInfo's public-release shape is the privacy boundary. Only the fields
    // explicitly listed by the endpoint contract are copied into this JSON.
    let sensors = info
        .map(|value| {
            value
                .sensors
                .iter()
                .filter(|sensor| sensor.value.is_finite())
                .take(128)
                .map(|sensor| {
                    json!({
                        "name": clipped(&sensor.name, 128),
                        "value": sensor.value,
                        "unit": clipped(&sensor.unit, 24),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let payload = json!({
        "schema_version": 1,
        "app_version": env!("CARGO_PKG_VERSION"),
        "locale": locale,
        "gpu_name": info.and_then(|value| value.gpu_name.as_deref()).map(|value| clipped(value, 256)),
        "provider_error": info.and_then(|value| value.error.as_deref()).map(|value| clipped(value, 2048)),
        "sensors": sensors,
        "message": clipped(message.trim(), 16_384),
        "contact": contact,
        "consent": true,
    });
    let body =
        serde_json::to_vec(&payload).map_err(|error| SubmitError::Network(error.to_string()))?;
    if body.len() > MAX_PAYLOAD {
        return Err(SubmitError::PayloadTooLarge);
    }

    let agent = wide(&format!("GPU-Shark/{}", env!("CARGO_PKG_VERSION")));
    let host = wide(HOST);
    let verb = wide("POST");
    let path = wide(PATH);
    let headers = wide("Content-Type: application/json; charset=utf-8\r\n");
    unsafe {
        let session = InternetHandle(WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ));
        if session.0.is_null() {
            return Err(SubmitError::Network("WinHTTP session failed".into()));
        }
        let connection = InternetHandle(WinHttpConnect(session.0, host.as_ptr(), 443, 0));
        if connection.0.is_null() {
            return Err(SubmitError::Network("HTTPS connection failed".into()));
        }
        let request = InternetHandle(WinHttpOpenRequest(
            connection.0,
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err(SubmitError::Network("HTTPS request failed".into()));
        }
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            body.as_ptr().cast(),
            body.len() as u32,
            body.len() as u32,
            0,
        ) == 0
        {
            return Err(SubmitError::Network("Could not send report".into()));
        }
        if WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0 {
            return Err(SubmitError::Network(
                "Could not receive server response".into(),
            ));
        }
        let mut status = 0u32;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut status_size,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(SubmitError::Network("Invalid server status".into()));
        }

        let mut response = Vec::new();
        loop {
            let mut chunk = [0u8; 4096];
            let mut read = 0u32;
            if WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            ) == 0
            {
                return Err(SubmitError::Network(
                    "Could not read server response".into(),
                ));
            }
            if read == 0 {
                break;
            }
            response.extend_from_slice(&chunk[..read as usize]);
            if response.len() > 64 * 1024 {
                return Err(SubmitError::Network("Server response is too large".into()));
            }
        }

        match status {
            201 => {
                let accepted: AcceptedResponse =
                    serde_json::from_slice(&response).map_err(|error| {
                        SubmitError::Network(format!("Invalid success response: {error}"))
                    })?;
                if accepted.status != "accepted" || accepted.report_id.trim().is_empty() {
                    return Err(SubmitError::Network("Invalid report confirmation".into()));
                }
                Ok(accepted.report_id)
            }
            400 => Err(SubmitError::InvalidPayload(save_rejected_payload(
                &body, &response,
            ))),
            413 => Err(SubmitError::PayloadTooLarge),
            429 => Err(SubmitError::RateLimited),
            500..=599 => Err(SubmitError::Server),
            code => Err(SubmitError::Network(format!(
                "Unexpected HTTP status {code}"
            ))),
        }
    }
}
