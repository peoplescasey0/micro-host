//! Host-side VFIO PCI/GPU passthrough state and Easy-GPU-PV (Hyper-V GPU Partitioning) synthesizer.
//!
//! Ported from:
//! - `integrations/sandbox/nvidia-openshell/crates/openshell-vfio/src/{pci.rs,gpu.rs}`
//! - `integrations/hypervisor/easy-gpu-pv/Add-VMGpuPartitionAdapterFiles.psm1`
//! - `TOOLBOX_INVENTORY.md` (App Sandbox & Easy-GPU-PV for GTX 1650)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const NVIDIA_VENDOR_ID: &str = "0x10de";
pub const GPU_CLASS_DISPLAY_VGA: &str = "0x030000";
pub const GPU_CLASS_DISPLAY_3D: u32 = 0x0302;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PciInfo {
    pub bdf: String,
    pub name: String,
    pub vendor: String,
    pub device: String,
    pub class: String,
    pub iommu_group: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PciBinding {
    pub bdf: String,
    pub sandbox_id: String,
    pub bound_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PciBindState {
    pub bindings: Vec<PciBinding>,
}

impl PciBindState {
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let data = fs::read_to_string(path)?;
        serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &data)?;
        fs::rename(&tmp, path)
    }
}

pub fn is_gpu_class(class_str: &str) -> bool {
    if class_str == GPU_CLASS_DISPLAY_VGA {
        return true;
    }
    if let Some(hex) = class_str.strip_prefix("0x") {
        if let Ok(val) = u32::from_str_radix(hex, 16) {
            return (val >> 8) == GPU_CLASS_DISPLAY_3D;
        }
    }
    false
}

pub fn validate_iommu_group_slice(bdfs: &[&str]) -> Result<(), String> {
    if bdfs.is_empty() {
        return Err("Cannot bind empty IOMMU group slice".to_string());
    }
    for bdf in bdfs {
        let parts: Vec<&str> = bdf.split(':').collect();
        if parts.len() < 2 {
            return Err(format!("Malformed BDF '{}'", bdf));
        }
    }
    Ok(())
}

/// Easy-GPU-PV (and App Sandbox) partition configuration for Windows VM GPU acceleration
/// (e.g. NVIDIA GTX 1650).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuPvPartitionConfig {
    pub vm_name: String,
    pub gpu_name: String,
    pub partition_vram_pct: u8,
}

impl GpuPvPartitionConfig {
    pub fn new(vm_name: &str, gpu_name: &str, vram_pct: u8) -> Result<Self, String> {
        if vram_pct == 0 || vram_pct > 100 {
            return Err("GPU partition VRAM percentage must be between 1 and 100".to_string());
        }
        Ok(Self {
            vm_name: vm_name.to_string(),
            gpu_name: gpu_name.to_string(),
            partition_vram_pct: vram_pct,
        })
    }

    pub fn to_powershell_commands(&self) -> Vec<String> {
        vec![
            format!(
                "Add-VMGpuPartitionAdapter -VMName \"{}\"",
                self.vm_name
            ),
            format!(
                "Set-VMGpuPartitionAdapter -VMName \"{}\" -MinPartitionVRAM 1 -MaxPartitionVRAM 1000000000 -OptimalPartitionVRAM 100000000",
                self.vm_name
            ),
            format!(
                "Set-VM -GuestControlledCacheTypes $true -LowMemoryMappedIoSpace 1Gb -HighMemoryMappedIoSpace 32Gb -VMName \"{}\"",
                self.vm_name
            ),
        ]
    }
}
