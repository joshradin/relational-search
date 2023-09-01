//! A user is just a representation of a person

/// A user struct
#[derive(Debug)]
pub struct User {
    name: String,
}

impl User {
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A user factory
#[derive(Debug)]
pub struct UserFactory;

impl UserFactory {
    /// Creates a user object
    pub fn create(&self, name: &str) -> User {
        User {
            name: name.to_string(),
        }
    }
}
