//! Zero-trust sandbox isolation contract for Post Card Spatial OS.
//!
//! Ported from:
//! - `integrations/sandbox/nvidia-openshell/crates/openshell-isolation-interface/src/contract.rs`
//! - `integrations/sandbox/opensandbox`

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendErrorKind {
    Invalid,
    Denied,
    Unavailable,
    Unsupported,
    Failed,
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    Descriptor(String),
    Denied(String),
    Unavailable(String),
    Unsupported(String),
    Attach(String),
    Confirm(String),
    Process(String),
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        if uid == 0 || gid == 0 || supplementary_gids.contains(&0) {
            return Err(BackendError::Descriptor(
                "workload identity must not contain UID or GID zero".to_string(),
            ));
        }
        if source.trim().is_empty() || resource_digest.trim().is_empty() {
            return Err(BackendError::Descriptor(
                "workload identity source and resource digest are required".to_string(),
            ));
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

/// Generation-bound provisioning evidence (`openshell-isolation-interface/src/contract.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningEvidence {
    pub generation: String,
    pub evidence_digest: [u8; 32],
}

impl ProvisioningEvidence {
    pub fn from_enforcement_evidence(
        generation: &str,
        native_evidence: &[u8],
    ) -> Result<Self, BackendError> {
        if generation.is_empty() || native_evidence.is_empty() {
            return Err(BackendError::Descriptor(
                "provisioning generation and native evidence are required".to_string(),
            ));
        }
        let mut digest = [0u8; 32];
        // Deterministic 256-bit mixing across 4x64-bit FNV-1a lanes
        for (lane, chunk) in digest.chunks_exact_mut(8).enumerate() {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325u64.wrapping_add(lane as u64 * 0x9e37_79b9);
            for &b in generation.as_bytes().iter().chain(native_evidence.iter()) {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            chunk.copy_from_slice(&h.to_be_bytes());
        }
        Ok(Self {
            generation: generation.to_string(),
            evidence_digest: digest,
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

/// Loopback-only target inside the sandbox boundary (`openshell-isolation-interface/src/contract.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpOpenDenial {
    PolicyDenied,
    IdentityUnavailable,
    InvalidDestination,
    ResourceExhausted,
    MediationUnavailable,
}

/// Typed supervisor decision for one pending TCP open (`contract.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpOpenDecision {
    RelayReady,
    Denied(TcpOpenDenial),
}
