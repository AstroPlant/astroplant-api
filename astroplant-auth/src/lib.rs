//! Implements authentication functionality for users and kits in the AstroPlant system.
//!
//! Note that there are some differences between user passwords and kit passwords.
//! Firstly, this module is used to validate user passwords but not kit passwords.
//! Secondly, kit password hashes are generated as to be compatible with mosquitto-auth-plug.

pub mod random;
pub mod hash;
pub mod token;
