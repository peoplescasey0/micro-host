//! PairDrop 6-digit pairing / 5-letter room relay & LocalSend v2 metadata.
//!
//! Ported from:
//! - `integrations/network-transfer/pairdrop/server/ws-server.js`
//! - `integrations/network-transfer/pairdrop/pairdrop-cli/pairdrop`
//! - LocalSend Protocol v2

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairDropRoomBinding {
    pub pair_key: String,
    pub public_room_id: String,
    pub creator_card_id: String,
    pub peers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSendPeerInfo {
    pub alias: String,
    pub version: String,
    pub device_model: String,
    pub device_type: String,
    pub port: u16,
    pub download: bool,
}

impl Default for LocalSendPeerInfo {
    fn default() -> Self {
        Self {
            alias: "Post Card Spatial OS".to_string(),
            version: "2.1".to_string(),
            device_model: "Post Card Hypervisor".to_string(),
            device_type: "desktop".to_string(),
            port: 53317,
            download: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct PairDropRelay {
    by_pair_key: BTreeMap<String, PairDropRoomBinding>,
}

impl PairDropRelay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initiate_pairing(
        &mut self,
        pair_key: &str,
        public_room_id: &str,
        creator_card_id: &str,
    ) -> Result<PairDropRoomBinding, String> {
        if pair_key.len() != 6 || !pair_key.bytes().all(|b| b.is_ascii_digit()) {
            return Err("pair_key must be a 6-digit numeric string".to_string());
        }
        if public_room_id.len() != 5 || !public_room_id.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err("public_room_id must be a 5-letter lowercase string".to_string());
        }
        let binding = PairDropRoomBinding {
            pair_key: pair_key.to_string(),
            public_room_id: public_room_id.to_string(),
            creator_card_id: creator_card_id.to_string(),
            peers: vec![creator_card_id.to_string()],
        };
        self.by_pair_key.insert(pair_key.to_string(), binding.clone());
        Ok(binding)
    }

    pub fn join_with_pair_key(
        &mut self,
        pair_key: &str,
        peer_card_id: &str,
    ) -> Result<PairDropRoomBinding, String> {
        let Some(binding) = self.by_pair_key.get_mut(pair_key) else {
            return Err("pair-device-join-key-invalid".to_string());
        };
        if !binding.peers.contains(&peer_card_id.to_string()) {
            binding.peers.push(peer_card_id.to_string());
        }
        Ok(binding.clone())
    }
}
