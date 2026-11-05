//! Mac authentication uses a device-bound enclave key. Passkey unlock uses the OS credential provider’s PRF.
use crate::vault::{Key, R};
#[cfg(target_os = "macos")]
mod native {
    extern "C" {
        pub fn rv_passkey_available() -> i32;
        pub fn rv_passkey_enroll(key: *const u8, record: *mut u8, capacity: usize) -> usize;
        pub fn rv_passkey_open(record: *const u8, len: usize, key: *mut u8) -> i32;
        pub fn rv_device_available() -> i32;
        pub fn rv_device_enroll(key: *const u8, record: *mut u8, capacity: usize) -> usize;
        pub fn rv_device_open(record: *const u8, len: usize, key: *mut u8) -> i32;
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Mac,
    Passkey,
}
impl Method {
    pub fn table(self) -> &'static str {
        match self {
            Self::Mac => "device_unlock",
            Self::Passkey => "passkey_unlock",
        }
    }
    pub fn error(self) -> &'static str {
        match self {
            Self::Mac => "device",
            Self::Passkey => "passkey",
        }
    }
    pub fn available(self) -> bool {
        #[cfg(target_os = "macos")]
        {
            unsafe {
                (match self {
                    Self::Mac => native::rv_device_available(),
                    Self::Passkey => native::rv_passkey_available(),
                }) == 1
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    pub fn enroll(self, key: &Key) -> R<Vec<u8>> {
        #[cfg(target_os = "macos")]
        {
            let mut record = vec![0u8; 8192];
            // FFI writes at most capacity bytes; key is exactly 32 bytes.
            let n = unsafe {
                (match self {
                    Self::Mac => native::rv_device_enroll,
                    Self::Passkey => native::rv_passkey_enroll,
                })(key.as_ptr(), record.as_mut_ptr(), record.len())
            };
            if n > 0 && n <= record.len() {
                record.truncate(n);
                return Ok(record);
            }
        }
        let _ = key;
        Err(self.error().into())
    }
    pub fn open(self, record: &[u8]) -> R<Key> {
        #[cfg(target_os = "macos")]
        if !record.is_empty() && record.len() <= 8192 {
            let mut key = zeroize::Zeroizing::new([0; 32]);
            if unsafe {
                (match self {
                    Self::Mac => native::rv_device_open,
                    Self::Passkey => native::rv_passkey_open,
                })(record.as_ptr(), record.len(), key.as_mut_ptr())
            } == 1
            {
                return Ok(key);
            }
        }
        let _ = record;
        Err(self.error().into())
    }
}
