extern crate ring;
extern crate data_encoding;

use crate::errors::EngineError;

const SALT : &str = "Hh_XSJN!";

pub fn hash_password(password: &str) -> String {
    let mut pbkdf2_hash = [0u8; ring::digest::SHA512_256_OUTPUT_LEN];

    ring::pbkdf2::derive(
        &ring::digest::SHA512,
        std::num::NonZeroU32::new(20000).unwrap(), 
        SALT.as_bytes(), 
        password.as_bytes(), 
        &mut pbkdf2_hash
    );

    data_encoding::HEXUPPER.encode(&pbkdf2_hash)
}

pub fn verify_password(password: &str, hashed_encoded_password: &str) -> Result<(), EngineError> {
    let decoded_bytes = data_encoding::HEXUPPER.decode(hashed_encoded_password.as_bytes()).map_err(|_| EngineError::Unauthorized("数据库内的密码无法正常解码，服务器内部错误。".to_owned()))?;
    ring::pbkdf2::verify(
        &ring::digest::SHA512,
        std::num::NonZeroU32::new(20000).unwrap(), 
        SALT.as_bytes(), 
        password.as_bytes(), 
        &decoded_bytes
    ).map_err(|_| EngineError::Unauthorized("密码校验不正确，没有权限访问。".to_owned()))
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_hash() {
        assert!(verify_password("abcdefghijklmn", &dbg!(hash_password("abcdefghijklmn"))[..]).is_ok());
        assert!(dbg!(verify_password("somethingthatisapassword", "IS_not_HEX_encoded_Thing")).is_err());
        assert!(dbg!(verify_password("goodbye", &hash_password("abcdefghijklmn"))).is_err());
    }
}
