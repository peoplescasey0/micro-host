//! Zero-trust sandbox isolation contract for Post Card Spatial OS.
//!
//! Ported and adapted from:
//! - `integrations/sandbox/nvidia-openshell/crates/openshell-isolation-interface/src/contract.rs`
//! - `integrations/sandbox/opensandbox` (OpenSandbox 1.2.0)

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendErrorKind {
    Invalid,
    Denied,
    Unavailable,
    Unsupported,
    Failed,
    Terminated,
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendError {
    #[error("Invalid descriptor: {0}")]
    Descriptor(String),
    #[error("Access denied by policy: {0}")]
    Denied(String),
    #[error("Resource unavailable: {0}")]
    Unavailable(String),
    #[error("Operation unsupported: {0}")]
    Unsupported(String),
    #[error("Sandbox attach failure: {0}")]
    Attach(String),
    #[error("Confirmation failed: {0}")]
    Confirm(String),
    #[error("Process failure: {0}")]
    Process(String),
    #[error("Sandbox terminated: {0}")]
    Terminated(String),
}

impl BackendError {
    pub fn kind(&self) -> BackendErrorKind {
        match self {
            Self::Descriptor(_) => BackendErrorKind::Invalid,
            Self::Denied(_) => BackendErrorKind::Denied,
            Self::Unavailable(_) => BackendErrorKind::Unavailable,
            Self::Unsupported(_) => BackendErrorKind::Unsupported,
            Self::Attach(_) | Self::Confirm(_) | Self::Process(_) => BackendErrorKind::Failed,
            Self::Terminated(_) => BackendErrorKind::Terminated,
        }
    }
}

/// Exact non-root workload identity validated before a sandbox is admitted
/// (`openshell-isolation-interface/src/contract.rs`).
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
    ) -> Result<Self, BackendError> {
        // Enforce zero-trust non-root requirement: root (UID 0) execution is strictly prohibited.
        if uid == 0 {
            return Err(BackendError::Denied(
                "workload identity with root uid 0 is prohibited by sandbox policy".to_string(),
            ));
        }
        if source.trim().is_empty() {
            return Err(BackendError::Descriptor(
                "workload identity source policy cannot be empty".to_string(),
            ));
        }

        supplementary_gids.retain(|&g| g != gid && g != 0);
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

/// Signed attestation evidence confirming workload was provisioned within policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningEvidence {
    pub generation: String,
    pub evidence_digest: String,
}

impl ProvisioningEvidence {
    pub fn from_enforcement_evidence(
        generation: &str,
        raw_evidence: &[u8],
    ) -> Result<Self, BackendError> {
        if generation.trim().is_empty() {
            return Err(BackendError::Descriptor(
                "attestation generation identifier cannot be empty".to_string(),
            ));
        }
        let mut hasher = Sha3_256::new();
        hasher.update(raw_evidence);
        let hash = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for byte in hash {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{:02x}", byte);
        }

        Ok(Self {
            generation: generation.to_string(),
            evidence_digest: format!("sha3-256:{}", hex),
        })
    }

    pub fn validate(&self, expected_generation: &str) -> Result<(), BackendError> {
        if !self.generation.is_empty() && self.generation == expected_generation {
            Ok(())
        } else {
            Err(BackendError::Confirm(
                "provisioning evidence belongs to another generation".to_string(),
            ))
        }
    }
}

/// Loopback-only target inside the sandbox boundary (`contract.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopbackTarget {
    octets: [u8; 4],
    port: u16,
}

impl LoopbackTarget {
    pub fn new(octets: [u8; 4], port: u16) -> Result<Self, BackendError> {
        if octets[0] != 127 || port == 0 {
            return Err(BackendError::Process(format!(
                "port-forward target {}.{}.{}.{}:{} is not loopback",
                octets[0], octets[1], octets[2], octets[3], port
            )));
        }
        Ok(Self { octets, port })
    }

    pub fn octets(&self) -> [u8; 4] {
        self.octets
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Placement-neutral reason why a staged TCP open was denied (`contract.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TcpOpenDenial {
    PolicyDenied,
    IdentityUnavailable,
    InvalidDestination,
    ResourceExhausted,
    MediationUnavailable,
}

/// Typed supervisor decision for one pending TCP open (`contract.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TcpOpenDecision {
    RelayReady,
    Denied(TcpOpenDenial),
}
