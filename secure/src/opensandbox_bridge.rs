//! Host-side OpenShell & OpenSandbox isolation contract verification.
//!
//! Ported from:
//! - `integrations/sandbox/nvidia-openshell/crates/openshell-isolation-interface/src/contract.rs`
//! - `integrations/sandbox/opensandbox`

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedWorkloadIdentity {
    pub uid: u32,
    pub gid: u32,
    pub supplementary_gids: Vec<u32>,
    pub source: String,
    pub resource_digest: String,
}

impl ResolvedWorkloadIdentity {
    pub fn new(
        uid: u32,
        gid: u32,
        mut supplementary_gids: Vec<u32>,
        source: &str,
        resource_digest: &str,
    ) -> Result<Self, String> {
        if uid == 0 || gid == 0 || supplementary_gids.contains(&0) {
            return Err("workload identity must not contain UID or GID zero".to_string());
        }
        if source.trim().is_empty() || resource_digest.trim().is_empty() {
            return Err("workload identity source and resource digest are required".to_string());
        }
        supplementary_gids.retain(|&g| g != gid);
        supplementary_gids.sort_unstable();
        supplementary_gids.dedup();
        Ok(Self {
            uid,
            gid,
            supplementary_gids,
            source: source.to_string(),
            resource_digest: resource_digest.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningEvidence {
    pub generation: String,
    pub evidence_digest_hex: String,
}

impl ProvisioningEvidence {
    pub fn from_enforcement_evidence(
        generation: &str,
        native_evidence: &[u8],
    ) -> Result<Self, String> {
        if generation.is_empty() || native_evidence.is_empty() {
            return Err("provisioning generation and native evidence are required".to_string());
        }
        let mut hasher = Sha3_256::new();
        hasher.update(&(generation.len() as u64).to_be_bytes());
        hasher.update(generation.as_bytes());
        hasher.update(native_evidence);
        let digest = hasher.finalize();
        let hex = digest.iter().map(|b| format!("{b:02x}")).collect();
        Ok(Self {
            generation: generation.to_string(),
            evidence_digest_hex: hex,
        })
    }

    pub fn validate(&self, expected_generation: &str) -> Result<(), String> {
        if !self.generation.is_empty() && self.generation == expected_generation {
            Ok(())
        } else {
            Err("provisioning evidence belongs to another generation".to_string())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopbackTarget {
    host: IpAddr,
    port: u16,
}

impl LoopbackTarget {
    pub fn new(host: IpAddr, port: u16) -> Result<Self, String> {
        if !host.is_loopback() || port == 0 {
            return Err(format!("port-forward target {host}:{port} is not loopback"));
        }
        Ok(Self { host, port })
    }

    pub fn host(&self) -> IpAddr {
        self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}
