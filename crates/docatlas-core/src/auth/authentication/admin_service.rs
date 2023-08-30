use std::fs::File;
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use argon2::password_hash::PasswordHashString;
use argon2::{Argon2, PasswordVerifier};
use base64::Engine;

use crate::auth::authentication::{
    write_password, AuthenticationError, AuthenticationRequest, AuthenticationRequestPayload,
    AuthenticationService,
};
use crate::auth::users::{User, UserFactory};

pub const DEFAULT_USER: &str = "admin";
pub const DEFAULT_PASSWORD: &[u8] = b"admin";

#[derive(Debug)]
pub struct AdminAuthenticationService {
    hashed_password: PasswordHashString,
    file_path: PathBuf,
}

impl AdminAuthenticationService {
    /// Open an admin auth service at a given path
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let hashed = std::fs::read(path.as_ref())?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(hashed)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
        Ok(Self {
            hashed_password: PasswordHashString::new(&String::from_utf8_lossy(&decoded))
                .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?,
            file_path: path.as_ref().to_path_buf(),
        })
    }

    /// create an admin auth service at a given path with a password, or the default `admin` password
    /// if none is supplied. If the file already exists, an error is returned.
    pub fn create<P: AsRef<Path>, A: Into<Option<Vec<u8>>>>(
        path: P,
        password: A,
    ) -> io::Result<Self> {
        let path = path.as_ref();
        {
            let file = File::options().write(true).create_new(true).open(path)?;

            write_password(file, password.into().as_deref().unwrap_or(DEFAULT_PASSWORD))?;
        }
        Self::open(path)
    }
}

impl AuthenticationService for AdminAuthenticationService {
    fn authenticate(&self, req: &AuthenticationRequest) -> Result<User, AuthenticationError> {
        let mut admin_check = false;
        for pass in req.payloads().filter_map(|req| match req {
            AuthenticationRequestPayload::Basic { username, password }
                if *username == DEFAULT_USER =>
            {
                Some(*password)
            }
            _ => None,
        }) {
            admin_check = true;
            if let Ok(()) = Argon2::default()
                .verify_password(pass.as_bytes(), &self.hashed_password.password_hash())
            {
                return Ok(UserFactory.create("admin"));
            }
        }

        if admin_check {
            Err(AuthenticationError::WrongPassword)
        } else {
            Err(AuthenticationError::UnknownIdentifier)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_admin() {
        let tempdir = tempdir().unwrap();
        let svc = AdminAuthenticationService::create(tempdir.path().join("admin"), None).unwrap();

        let ref request = AuthenticationRequest::new().with_basic("admin", "admin");
        let admin = svc.authenticate(request).unwrap();
        assert_eq!(admin.name(), "admin");
    }

    #[test]
    fn only_accept_admin_check() {
        let tempdir = tempdir().unwrap();
        let svc = AdminAuthenticationService::create(tempdir.path().join("admin#1"), None).unwrap();

        let ref request = AuthenticationRequest::new().with_basic("blahblagbalg", "admin");
        let error = svc.authenticate(request).unwrap_err();
        assert!(matches!(error, AuthenticationError::UnknownIdentifier));
    }
}
