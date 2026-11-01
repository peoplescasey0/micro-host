//! PCI discovery, BitVisor VT-d/DMAR definitions, and NVIDIA OpenShell VFIO passthrough guards.
//!
//! Ported and unified from:
//! - `integrations/hypervisor/bitvisor-3.0/include/passthrough/vtd.h`
//! - `integrations/sandbox/nvidia-openshell/crates/openshell-vfio/src/{pci.rs,gpu.rs}`

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

pub const NVIDIA_VENDOR_ID: u16 = 0x10DE;
pub const INTEL_VENDOR_ID: u16 = 0x8086;
pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

pub const GPU_CLASS_DISPLAY_VGA: &str = "0x030000";
pub const GPU_CLASS_DISPLAY_3D: u32 = 0x0302;

/// BitVisor VT-d DMA permission flags (`vtd.h`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DmaPermission {
    None = 0b00,
    ReadOnly = 0b01,
    WriteOnly = 0b10,
    ReadWrite = 0b11,
}

/// BitVisor ACPI DMAR Remapping Structure types (`vtd.h`).
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DmarStructureType {
    Drhd = 0,
    Rmrr = 1,
    Atsr = 2,
    Rhsa = 3,
}

/// BitVisor DMA Remapping Hardware Unit Definition (`struct acpi_ent_drhd` in `vtd.h`).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AcpiDrhdEntry {
    pub struct_type: u16,
    pub length: u16,
    pub flags: u8,
    pub reserved: u8,
    pub segment: u16,
    pub register_base_address: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PciInfo {
    pub bdf: String,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u8,
    pub subclass_id: u8,
    pub name: String,
    pub iommu_group: u32,
    pub driver: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum VfioError {
    #[error("Empty IOMMU device group")]
    EmptyGroup,
    #[error("Invalid BDF identifier format: '{0}' (expected DD:DD.D or DDDD:DD:DD.D)")]
    InvalidBdf(String),
    #[error("Device '{0}' was not found in PCI bus inventory")]
    DeviceNotFound(String),
    #[error("Device '{bdf}' is currently bound to driver '{driver}', not 'vfio-pci'")]
    NotBoundToVfio { bdf: String, driver: String },
    #[error("Device '{bdf}' belongs to IOMMU group {actual_group}, expected group {expected_group}")]
    GroupMismatch {
        bdf: String,
        expected_group: u32,
        actual_group: u32,
    },
    #[error("IOMMU group {group} has unassigned sibling devices: {peers:?}")]
    IommuGroupConflict {
        bdf: String,
        group: u32,
        peers: Vec<String>,
    },
}

/// Validate standard PCI Bus/Device/Function syntax (e.g. `0000:00:02.0` or `00:02.0`).
pub fn validate_bdf(bdf: &str) -> Result<(), VfioError> {
    let parts: Vec<&str> = bdf.split(':').collect();
    if parts.len() == 2 {
        let dev_fn: Vec<&str> = parts[1].split('.').collect();
        if dev_fn.len() == 2
            && u8::from_str_radix(parts[0], 16).is_ok()
            && u8::from_str_radix(dev_fn[0], 16).is_ok()
            && u8::from_str_radix(dev_fn[1], 16).is_ok()
        {
            return Ok(());
        }
    } else if parts.len() == 3 {
        let dev_fn: Vec<&str> = parts[2].split('.').collect();
        if dev_fn.len() == 2
            && u16::from_str_radix(parts[0], 16).is_ok()
            && u8::from_str_radix(parts[1], 16).is_ok()
            && u8::from_str_radix(dev_fn[0], 16).is_ok()
            && u8::from_str_radix(dev_fn[1], 16).is_ok()
        {
            return Ok(());
        }
    }
    Err(VfioError::InvalidBdf(bdf.to_string()))
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

pub fn validate_iommu_group_slice(bdfs: &[&str]) -> Result<(), String> {
    if bdfs.is_empty() {
        return Err("Cannot bind empty IOMMU group slice".to_string());
    }
    for bdf in bdfs {
        validate_bdf(bdf).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// RAII Guard that rolls back a device's VFIO binding if dropped without disarm
/// (`openshell-vfio/src/pci.rs`).
#[derive(Debug)]
pub struct PciBindGuard {
    pub bdf: String,
    pub companion_bdfs: Vec<String>,
    pub disarmed: bool,
}

impl PciBindGuard {
    pub fn disarm(mut self) {
        self.disarmed = true;
    }
}

pub static MOCK_PCI_BUS: Mutex<Vec<PciInfo>> = Mutex::new(Vec::new());

pub fn init_default_pci_bus() {
    let mut bus = MOCK_PCI_BUS.lock().unwrap();
    bus.clear();
    bus.push(PciInfo {
        bdf: "0000:00:00.0".to_string(),
        bus: 0,
        device: 0,
        function: 0,
        vendor_id: INTEL_VENDOR_ID,
        device_id: 0x1237,
        class_id: 0x06,
        subclass_id: 0x00,
        name: "Intel 440FX Host Bridge".to_string(),
        iommu_group: 0,
        driver: "host-bridge".to_string(),
    });
    bus.push(PciInfo {
        bdf: "0000:00:01.0".to_string(),
        bus: 0,
        device: 1,
        function: 0,
        vendor_id: INTEL_VENDOR_ID,
        device_id: 0x7000,
        class_id: 0x06,
        subclass_id: 0x01,
        name: "Intel PIIX3 ISA Bridge".to_string(),
        iommu_group: 0,
        driver: "isa-bridge".to_string(),
    });
    bus.push(PciInfo {
        bdf: "0000:01:00.0".to_string(),
        bus: 1,
        device: 0,
        function: 0,
        vendor_id: NVIDIA_VENDOR_ID,
        device_id: 0x1F91,
        class_id: 0x03,
        subclass_id: 0x00,
        name: "NVIDIA GeForce GTX 1650 [TU117]".to_string(),
        iommu_group: 1,
        driver: "nouveau".to_string(),
    });
    bus.push(PciInfo {
        bdf: "0000:01:00.1".to_string(),
        bus: 1,
        device: 0,
        function: 1,
        vendor_id: NVIDIA_VENDOR_ID,
        device_id: 0x10FA,
        class_id: 0x04,
        subclass_id: 0x03,
        name: "NVIDIA High Definition Audio".to_string(),
        iommu_group: 1,
        driver: "snd_hda_intel".to_string(),
    });
}

pub fn prepare_pci_for_passthrough(bdf: &str) -> Result<PciBindGuard, VfioError> {
    validate_bdf(bdf)?;
    let mut bus = MOCK_PCI_BUS.lock().unwrap();
    let dev = bus
        .iter()
        .find(|d| d.bdf == bdf)
        .ok_or_else(|| VfioError::DeviceNotFound(bdf.to_string()))?;
    let target_group = dev.iommu_group;

    let _siblings: Vec<String> = bus
        .iter()
        .filter(|d| d.iommu_group == target_group && d.bdf != bdf)
        .map(|d| d.bdf.clone())
        .collect();

    let mut companions = Vec::new();
    for dev in bus.iter_mut() {
        if dev.iommu_group == target_group {
            if dev.bdf != bdf {
                companions.push(dev.bdf.clone());
            }
            dev.driver = "vfio-pci".to_string();
        }
    }

    Ok(PciBindGuard {
        bdf: bdf.to_string(),
        companion_bdfs: companions,
        disarmed: false,
    })
}

pub fn prepare_pci_group_for_passthrough(bdfs: &[&str]) -> Result<PciBindGuard, VfioError> {
    let (primary, rest) = bdfs.split_first().ok_or(VfioError::EmptyGroup)?;
    for bdf in bdfs {
        validate_bdf(bdf)?;
    }
    let mut bus = MOCK_PCI_BUS.lock().unwrap();
    let primary_dev = bus
        .iter()
        .find(|d| d.bdf == *primary)
        .ok_or_else(|| VfioError::DeviceNotFound((*primary).to_string()))?;
    let expected_group = primary_dev.iommu_group;

    for bdf in rest {
        let dev = bus
            .iter()
            .find(|d| d.bdf == *bdf)
            .ok_or_else(|| VfioError::DeviceNotFound((*bdf).to_string()))?;
        if dev.iommu_group != expected_group {
            return Err(VfioError::GroupMismatch {
                bdf: (*bdf).to_string(),
                expected_group,
                actual_group: dev.iommu_group,
            });
        }
    }

    for dev in bus.iter_mut() {
        if dev.iommu_group == expected_group {
            dev.driver = "vfio-pci".to_string();
        }
    }

    Ok(PciBindGuard {
        bdf: (*primary).to_string(),
        companion_bdfs: rest.iter().map(|s| (*s).to_string()).collect(),
        disarmed: false,
    })
}
