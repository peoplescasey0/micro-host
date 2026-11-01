//! Post Card Spatial OS — Secure Host Vault & Hardware Passthrough Library
//!
//! Integrates NVIDIA OpenShell VFIO, BitVisor 3.0 VT-d/DMAR structures,
//! Cosmos OS MBR storage access & spooling, OpenSandbox zero-trust isolation contracts,
//! and PairDrop/LocalSend cross-card transfer relays.

pub mod opensandbox_bridge;
pub mod pairdrop_relay;
pub mod pci;
pub mod sandbox;
pub mod storage;
pub mod vfio;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn pci_vfio_guards_and_bdf_validation_work() {
        assert!(pci::validate_bdf("00:02.0").is_ok());
        assert!(pci::validate_bdf("0000:01:00.0").is_ok());
        assert!(pci::validate_bdf("invalid-bdf").is_err());

        pci::init_default_pci_bus();
        let guard = pci::prepare_pci_for_passthrough("0000:01:00.0").unwrap();
        assert_eq!(guard.bdf, "0000:01:00.0");
        assert_eq!(guard.companion_bdfs, vec!["0000:01:00.1"]);
        guard.disarm();

        let group_guard =
            pci::prepare_pci_group_for_passthrough(&["0000:01:00.0", "0000:01:00.1"]).unwrap();
        assert_eq!(group_guard.bdf, "0000:01:00.0");
        group_guard.disarm();
    }

    #[test]
    fn storage_mbr_parsing_and_spooler_work() {
        // Construct a mock 512-byte MBR sector with 0x55AA boot signature
        let mut sector = vec![0u8; 512];
        sector[510] = 0x55;
        sector[511] = 0xAA;
        // Partition 1 at 0x1BE: bootable, type 0x83 (Linux), start LBA 2048, 102400 sectors
        sector[0x1BE] = 0x80;
        sector[0x1BE + 4] = 0x83;
        sector[0x1BE + 8..0x1BE + 12].copy_from_slice(&2048u32.to_le_bytes());
        sector[0x1BE + 12..0x1BE + 16].copy_from_slice(&102400u32.to_le_bytes());

        let parts = storage::parse_mbr_partitions(&sector).unwrap();
        assert_eq!(parts.len(), 1);
        assert!(parts[0].bootable);
        assert_eq!(parts[0].type_name(), "Linux ext4/xfs");
        assert_eq!(parts[0].start_lba, 2048);
        assert_eq!(parts[0].sector_count, 102400);
        assert_eq!(parts[0].size_bytes(), 102400 * 512);

        // Test FIFO I/O job spooler
        let access = storage::StorageAccess {
            lba: 2048,
            count: 4,
            sector_size: 512,
            rw: storage::StorageRw::Write,
        };
        let job_id = storage::submit_spool_job("disk0", access, vec![0x42; 2048]).unwrap();
        let (pending, _) = storage::spool_stats();
        assert!(pending >= 1);

        let completed = storage::process_next_spool_job().unwrap();
        assert_eq!(completed.id, job_id);
        assert_eq!(completed.state, storage::SpoolJobState::Completed);
    }

    #[test]
    fn sandbox_contracts_and_workload_identity_work() {
        // Root UID 0 must be rejected
        assert!(
            sandbox::ResolvedWorkloadIdentity::new(0, 1000, vec![], "policy", "sha3:1").is_err()
        );
        let id = sandbox::ResolvedWorkloadIdentity::new(
            1000,
            1000,
            vec![1000, 27, 44, 27],
            "card-policy",
            "sha3:1234",
        )
        .unwrap();
        assert_eq!(id.supplementary_gids, vec![27, 44]);

        let ev =
            sandbox::ProvisioningEvidence::from_enforcement_evidence("gen-1", b"tpm2+vfio")
                .unwrap();
        assert!(ev.validate("gen-1").is_ok());
        assert!(ev.validate("gen-2").is_err());

        let lo = sandbox::LoopbackTarget::new([127, 0, 0, 1], 8080);
        assert!(lo.is_ok());
        let non_lo = sandbox::LoopbackTarget::new([192, 168, 1, 1], 8080);
        assert!(non_lo.is_err());
    }

    #[test]
    fn vfio_bind_state_and_gpu_pv_work() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("pci-state.json");
        let state = vfio::PciBindState {
            bindings: vec![vfio::PciBinding {
                bdf: "0000:2d:00.0".to_string(),
                sandbox_id: "card-linux".to_string(),
                bound_at_ms: 1_700_000_000_000,
            }],
        };
        state.save(&path).unwrap();
        let loaded = vfio::PciBindState::load(&path).unwrap();
        assert_eq!(loaded, state);
        assert!(vfio::is_gpu_class("0x030000"));
        assert!(vfio::is_gpu_class("0x030200"));
        assert!(!vfio::is_gpu_class("0x020000"));
        assert!(vfio::validate_iommu_group_slice(&["0000:2d:00.0", "0000:2d:00.1"]).is_ok());

        let gpu_pv = vfio::GpuPvPartitionConfig::new("PostCard-Win11", "RTX", 50).unwrap();
        let cmds = gpu_pv.to_powershell_commands();
        assert_eq!(cmds.len(), 3);
        assert!(cmds[0].contains("Add-VMGpuPartitionAdapter"));
    }

    #[test]
    fn pairdrop_and_localsend_relay_work() {
        let mut relay = pairdrop_relay::PairDropRelay::new();
        let room = relay
            .initiate_pairing("482910", "kpxqm", "windows")
            .unwrap();
        assert_eq!(room.peers, vec!["windows"]);
        let joined = relay.join_with_pair_key("482910", "linux").unwrap();
        assert_eq!(joined.peers, vec!["windows", "linux"]);

        let ls = pairdrop_relay::LocalSendPeerInfo::default();
        assert_eq!(ls.port, 53317);
        assert!(ls.download);
    }
}
