use std::str::FromStr;

enum HashVersion {
    V1,
}

impl HashVersion {
    pub fn from_hash(hash: &str) -> Option<HashVersion> {
        let version: String = hash.chars().take_while(|c| c != &'$').collect();
        match version.as_ref() {
            "1" => Some(HashVersion::V1),
            _ => None,
        }
    }
}

/// Hashing algorithm V1, using PBKDF2 with 15_000 iterations and a 128-bit salt.
struct V1Hash {
    pub salt: [u8; 16],
    pub hash: [u8; 32],
}

impl V1Hash {
    const PBKDF2_ITERATIONS: u32 = 15_000;

    /// Hash a user password.
    pub fn hash_password(password: &str) -> Self {
        use rand::RngCore;

        let mut salt = [0u8; 16];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut salt);

        let hash = pbkdf2(password, &salt, V1Hash::PBKDF2_ITERATIONS);

        V1Hash { salt, hash }
    }

    pub fn check(&self, password: &str) -> bool {
        let hash = pbkdf2(password, &self.salt, V1Hash::PBKDF2_ITERATIONS);
        hash == self.hash
    }
}

impl FromStr for V1Hash {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.split('$').collect();
        if parts.len() != 3 {
            return Err(());
        }

        if parts[0] != "1" {
            return Err(());
        }

        let vec_salt = base64::decode(parts[1]).map_err(|_| ())?;
        let vec_hash = base64::decode(parts[2]).map_err(|_| ())?;

        if vec_salt.len() != 16 {
            return Err(());
        }

        if vec_hash.len() != 32 {
            return Err(());
        }

        let mut salt = [0u8; 16];
        salt.copy_from_slice(&vec_salt);

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&vec_hash);

        Ok(Self { salt, hash })
    }
}

impl ToString for V1Hash {
    fn to_string(&self) -> String {
        format!(
            "1${}${}",
            base64::encode(&self.salt),
            base64::encode(&self.hash)
        )
    }
}

fn kit_hash_format(iterations: u32, salt: &str, hash: &[u8]) -> String {
    format!(
        "PBKDF2$sha256${}${}${}",
        iterations,
        salt,
        base64::encode(&hash)
    )
}

/// Generate a mosquitto-auth-plug compatible PBKDF2 hash.
pub fn hash_kit_password(password: &str) -> String {
    const SALT_LENGTH: usize = 20;
    const PBKDF2_ITERATIONS: u32 = 15_000;

    let salt = random_string::string(SALT_LENGTH);
    let hash = pbkdf2(password, salt.as_bytes(), PBKDF2_ITERATIONS);

    kit_hash_format(PBKDF2_ITERATIONS, &salt, &hash)
}

/// Perform pbkdf2.
fn pbkdf2(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    use crypto::{hmac::Hmac, sha2::Sha256};
    let mut mac = Hmac::new(Sha256::new(), password.as_bytes());

    // The 256-bit derived key.
    let mut dk = [0u8; 32];

    crypto::pbkdf2::pbkdf2(&mut mac, salt, iterations, &mut dk);

    dk
}

/// Check a password against a hash previously generated by this crate.
pub fn check_user_password(password: &str, hash: &str) -> bool {
    match HashVersion::from_hash(hash) {
        Some(HashVersion::V1) => {
            let v1_hash: V1Hash = match hash.parse() {
                Ok(v1_hash) => v1_hash,
                Err(_) => return false,
            };

            v1_hash.check(password)
        }
        None => false,
    }
}

/// Hash a user password.
pub fn hash_user_password(password: &str) -> String {
    let v1_hash = V1Hash::hash_password(password);

    v1_hash.to_string()
}

#[cfg(test)]
mod test {
    #[test]
    pub fn known_hash() {
        assert_eq!(
            &base64::encode(&super::pbkdf2(
                "It all adds up to normality.",
                "Z416JHE8vSmaiamV5TRz".as_bytes(),
                2_000
            )),
            "z3y6FvWAZtyQe6TV+O/oyhC3oqnF8KJdlB5Lphi+Lwg="
        );
    }

    #[test]
    pub fn kit_hash_format() {
        assert_eq!(
            &super::kit_hash_format(
                2_000,
                "Z416JHE8vSmaiamV5TRz",
                &base64::decode("z3y6FvWAZtyQe6TV+O/oyhC3oqnF8KJdlB5Lphi+Lwg=").unwrap()
            ),
            "PBKDF2$sha256$2000$Z416JHE8vSmaiamV5TRz$z3y6FvWAZtyQe6TV+O/oyhC3oqnF8KJdlB5Lphi+Lwg="
        )
    }

    #[test]
    pub fn check_v1_hash() {
        let v1_hash: super::V1Hash =
            "1$AQIDBAUGBwgJEBESExQVFg==$CxZam+iAsfxNt9doaNmMtSjBy6NqyoOMxSppNpJFmx8="
                .parse()
                .unwrap();

        assert!(v1_hash.check("It all adds up to normality."),)
    }

    #[test]
    pub fn hash_round_trip() {
        let password = "It all adds up to normality.";
        let v1_hash = super::V1Hash::hash_password(password);
        assert!(v1_hash.check(password))
    }
}
