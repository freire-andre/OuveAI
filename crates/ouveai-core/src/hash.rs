//! SHA-256 pelo CNG do Windows (BCrypt), sem crate extra (decidido no S4).

use std::io::Read;
use std::path::Path;

pub struct Sha256 {
    alg: *mut core::ffi::c_void,
    hash: *mut core::ffi::c_void,
}

// Os handles do BCrypt podem mudar de thread; só não podem ser usados por duas ao mesmo tempo.
unsafe impl Send for Sha256 {}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        use windows_sys::Win32::Security::Cryptography::*;
        let (mut alg, mut hash) = (std::ptr::null_mut(), std::ptr::null_mut());
        unsafe {
            assert_eq!(BCryptOpenAlgorithmProvider(&mut alg, BCRYPT_SHA256_ALGORITHM, std::ptr::null(), 0), 0);
            assert_eq!(BCryptCreateHash(alg, &mut hash, std::ptr::null_mut(), 0, std::ptr::null(), 0, 0), 0);
        }
        Self { alg, hash }
    }

    pub fn update(&mut self, data: &[u8]) {
        use windows_sys::Win32::Security::Cryptography::BCryptHashData;
        for chunk in data.chunks(u32::MAX as usize) {
            unsafe { assert_eq!(BCryptHashData(self.hash, chunk.as_ptr(), chunk.len() as u32, 0), 0) };
        }
    }

    pub fn hex(self) -> String {
        use windows_sys::Win32::Security::Cryptography::BCryptFinishHash;
        let mut out = [0u8; 32];
        unsafe { assert_eq!(BCryptFinishHash(self.hash, out.as_mut_ptr(), 32, 0), 0) };
        out.iter().map(|b| format!("{b:02x}")).collect()
        // `Drop` libera os handles.
    }
}

impl Drop for Sha256 {
    fn drop(&mut self) {
        use windows_sys::Win32::Security::Cryptography::{BCryptCloseAlgorithmProvider, BCryptDestroyHash};
        unsafe {
            BCryptDestroyHash(self.hash);
            BCryptCloseAlgorithmProvider(self.alg, 0);
        }
    }
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.hex()
}

/// SHA-256 de um arquivo; `progress(lidos)` a cada bloco de 4 MB.
pub fn sha256_file(path: &Path, progress: &mut dyn FnMut(u64)) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 4 << 20];
    let mut done = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        done += n as u64;
        progress(done);
    }
    Ok(h.hex())
}

#[cfg(test)]
mod tests {
    #[test]
    fn vetor_conhecido() {
        assert_eq!(super::sha256_bytes(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(super::sha256_bytes(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}
