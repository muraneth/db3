//
// key.rs
// Copyright (C) 2022 db3.network Author imotai <codego.me@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use db3_crypto::db3_address::{DB3Address, DB3_ADDRESS_LENGTH};
use db3_error::{DB3Error, Result};
const NAMESPACE: &str = "_NS_";
const MAX_USE_KEY_LEN: usize = 128 * 4;
const MAX_NAMESPACE_LEN: usize = 16;
const MIN_KEY_TOTAL_LEN: usize = DB3_ADDRESS_LENGTH + NAMESPACE.len();

/// account_address + NAMESPACE + ns  + user_key
pub struct Key<'a>(pub DB3Address, pub &'a [u8], pub &'a [u8]);

impl<'a> Key<'a> {
    ///
    /// encode the key
    ///
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.1.len() > MAX_NAMESPACE_LEN || self.2.len() > MAX_USE_KEY_LEN {
            return Err(DB3Error::KeyCodecError(format!(
                "the length {} of namespace or key exceeds the limit",
                self.2.len()
            )));
        }
        let mut encoded_key = self.0.as_ref().to_vec();
        encoded_key.extend_from_slice(NAMESPACE.as_bytes());
        encoded_key.extend_from_slice(self.1);
        encoded_key.extend_from_slice(self.2);
        Ok(encoded_key)
    }

    ///
    /// decode the key
    ///
    pub fn decode(data: &'a [u8], ns: &'a [u8]) -> Result<Self> {
        if data.len() <= MIN_KEY_TOTAL_LEN {
            return Err(DB3Error::KeyCodecError(
                "the length of data is invalid".to_string(),
            ));
        }

        let key_start_offset = MIN_KEY_TOTAL_LEN + ns.len();
        let data_slice: &[u8; DB3_ADDRESS_LENGTH] = &data[..DB3_ADDRESS_LENGTH]
            .try_into()
            .map_err(|e| DB3Error::KeyCodecError(format!("{e}")))?;

        let addr = DB3Address::from(data_slice);
        Ok(Self(addr, ns, &data[key_start_offset..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db3_crypto::key_derive;
    use db3_crypto::signature_scheme::SignatureScheme;

    fn gen_address() -> DB3Address {
        let seed: [u8; 32] = [0; 32];
        let (address, _) =
            key_derive::derive_key_pair_from_path(&seed, None, &SignatureScheme::ED25519).unwrap();
        address
    }

    #[test]
    fn it_key_serde() {
        let addr = gen_address();
        let ns: &str = "ns1";
        let k: &str = "k1";
        let key = Key(addr, ns.as_bytes(), k.as_bytes());
        let key_encoded = key.encode();
        assert!(key_encoded.is_ok());
        let key_decoded = Key::decode(key_encoded.as_ref().unwrap(), ns.as_bytes());
        assert!(key_decoded.is_ok());
        assert!(key_decoded.unwrap().0 == addr);
    }

    #[test]
    fn it_key_serde_cmp() -> Result<()> {
        let addr = gen_address();
        let ns: &str = "ns1";
        let k: &str = "k1";
        let key = Key(addr, ns.as_bytes(), k.as_bytes());
        let key_encoded1 = key.encode()?;
        let ns: &str = "ns1";
        let k: &str = "k2";
        let key = Key(addr, ns.as_bytes(), k.as_bytes());
        let key_encoded2 = key.encode()?;
        assert!(key_encoded1.cmp(&key_encoded1) == std::cmp::Ordering::Equal);
        assert!(key_encoded1.cmp(&key_encoded2) == std::cmp::Ordering::Less);
        Ok(())
    }
}
