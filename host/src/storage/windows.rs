use std::{fs, io, path::PathBuf};

use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE, CryptProtectData, CryptUnprotectData,
        },
    },
    core::PCWSTR,
};

use super::{CredentialStore, HostIdentity};

/// Machine-scope DPAPI store for service credentials. The enclosing ProgramData
/// directory is additionally ACLed by the installer to Administrators/System.
pub struct WindowsCredentialStore {
    pub path: PathBuf,
}

impl WindowsCredentialStore {
    pub fn program_data() -> io::Result<Self> {
        let root = std::env::var_os("ProgramData")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ProgramData is unavailable"))?;
        Ok(Self {
            path: PathBuf::from(root)
                .join("DeckyPowerHost")
                .join("credentials.dpapi"),
        })
    }

    fn protect(plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(plaintext.len()).map_err(io::Error::other)?,
            pbData: plaintext.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE,
                &mut output,
            )
        }
        .map_err(io::Error::other)?;
        let result =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
        unsafe { LocalFree(Some(HLOCAL(output.pbData.cast()))) };
        Ok(result)
    }

    fn unprotect(ciphertext: &[u8]) -> io::Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(ciphertext.len()).map_err(io::Error::other)?,
            pbData: ciphertext.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe { CryptUnprotectData(&input, None, None, None, None, 0, &mut output) }
            .map_err(io::Error::other)?;
        let result =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
        unsafe { LocalFree(Some(HLOCAL(output.pbData.cast()))) };
        Ok(result)
    }
}

impl CredentialStore for WindowsCredentialStore {
    fn load_or_create(&self) -> io::Result<HostIdentity> {
        if !self.path.exists() {
            let identity = HostIdentity::default();
            self.save(&identity)?;
            return Ok(identity);
        }
        let plaintext = Self::unprotect(&fs::read(&self.path)?)?;
        serde_json::from_slice(&plaintext).map_err(io::Error::other)
    }
    fn save(&self, identity: &HostIdentity) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let plaintext = serde_json::to_vec(identity).map_err(io::Error::other)?;
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, Self::protect(&plaintext)?)?;
        fs::rename(temporary, &self.path)
    }
}
