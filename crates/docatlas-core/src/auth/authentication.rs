use crate::auth::authentication::admin_service::AdminAuthenticationService;
use crate::auth::users::User;
use argon2::password_hash::{PasswordHashString, Salt, SaltString};
use argon2::{PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine;
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, SecretVec};
use std::fmt::{Debug, Formatter};
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use thiserror::Error;

mod admin_service;

/// Writes a password by first hashing the password, then converting it into base64 encoding.
pub fn write_password<W: Write>(mut writer: W, password: impl AsRef<[u8]>) -> io::Result<()> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2::Argon2::default()
        .hash_password(password.as_ref(), &salt)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
    write!(
        writer,
        "{}",
        base64::engine::general_purpose::STANDARD.encode(hash.to_string())
    )
}

/// Reads a base64 encoded password hash
pub fn read_password_hash<R: Read>(mut reader: R) -> io::Result<PasswordHashString> {
    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(buffer)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;

    let parsed = String::from_utf8(bytes)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
    let password_hash = PasswordHashString::new(&parsed)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;

    Ok(password_hash)
}

/// An authentication toolchain consists of multiple of authentication services
pub struct AuthenticationToolchain {
    services: Vec<Box<dyn AuthenticationService>>,
}

impl Debug for AuthenticationToolchain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthenticationToolchain")
            .field("services", &self.services.len())
            .finish()
    }
}

impl AuthenticationToolchain {
    /// Creates a new authentication toolchain
    pub fn new(store_path: &Path) -> Self {
        let mut toolchain = Self { services: vec![] };
        toolchain.push(AdminAuthenticationService::open(store_path).unwrap());
        toolchain
    }

    /// Pushes a new authentication service to the end of the toolchain
    pub fn push(&mut self, auth: impl AuthenticationService + 'static) {
        self.services.push(Box::new(auth))
    }

    /// Tries to authenticate
    pub fn authenticate(
        &self,
        req: AuthenticationRequest,
    ) -> Result<User, Vec<AuthenticationError>> {
        let mut errs = Vec::new();
        'services_loop: for service in &self.services {
            let result = service.authenticate(&req);
            match result {
                Ok(user) => return Ok(user),
                Err(e) => {
                    let try_next = e.try_next();
                    errs.push(e);

                    if !try_next {
                        break 'services_loop;
                    }
                }
            }
        }
        Err(errs)
    }
}

/// Used for authenticating someone into the system
pub trait AuthenticationService {
    /// Authenticate a request
    fn authenticate(&self, req: &AuthenticationRequest) -> Result<User, AuthenticationError>;
}

/// An authentication request.
#[derive(Debug)]
pub struct AuthenticationRequest<'a> {
    payloads: Vec<AuthenticationRequestPayload<'a>>,
}

impl<'a> AuthenticationRequest<'a> {
    pub fn new() -> Self {
        Self { payloads: vec![] }
    }
    pub fn payloads(&self) -> impl Iterator<Item = &AuthenticationRequestPayload> {
        self.payloads.iter()
    }

    pub fn with_basic(mut self, username: &'a str, password: &'a str) -> Self {
        self.payloads
            .push(AuthenticationRequestPayload::Basic { username, password });
        self
    }
}

#[derive(Debug)]
pub enum AuthenticationRequestPayload<'a> {
    /// A basic request payload, containing a "basic" token.
    Basic {
        username: &'a str,
        password: &'a str,
    },
}

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error("The given request kind is unsupported")]
    UnsupportedRequestKind,
    #[error("Wrong password.")]
    WrongPassword,
    #[error("Unknown username.")]
    UnknownIdentifier,
}

impl AuthenticationError {
    fn try_next(&self) -> bool {
        match self {
            AuthenticationError::WrongPassword => false,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_basic() {
        let e = AuthenticationRequest::new().with_basic("username", "password");

        assert_eq!(e.payloads.len(), 1);
        let &AuthenticationRequestPayload::Basic { username, password } = &e.payloads[0] else {
            panic!("wrong payload")
        };

        assert_eq!(username, "username");
        assert_eq!(password, "password");
    }
}
