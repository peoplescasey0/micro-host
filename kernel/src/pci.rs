//! Bare-metal PCI enumeration, BitVisor VT-d/DMAR structures, and NVIDIA OpenShell VFIO
//! passthrough guard & IOMMU group validation.
//!
//! Ported from:
//! - `integrations/hypervisor/bitvisor-3.0/include/passthrough/vtd.h`
//! - `integrations/sandbox/nvidia-openshell/crates/openshell-vfio/src/{pci.rs,gpu.rs}`

use crate::sync::Mutex;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use x86_64::instructions::port::Port;

const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;
const PCI_CONFIG_DATA: u16 = 0x0CFC;

pub const NVIDIA_VENDOR_ID: u16 = 0x10DE;
pub const INTEL_VENDOR_ID: u16 = 0x8086;
pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

/// BitVisor VT-d DMA permission flags (`bitvisor-3.0/include/passthrough/vtd.h`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaPermission {
    None = 0b00,
    ReadOnly = 0b01,
    WriteOnly = 0b10,
    ReadWrite = 0b11,
}

/// BitVisor ACPI DMAR Remapping Structure types (`vtd.h`).
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Information about a discovered PCI device eligible for VFIO passthrough
/// (`openshell-vfio/src/pci.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PciInfo {
    pub bdf: String,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub iommu_group: u32,
    pub driver: &'static str,
}

impl PciInfo {
    /// Class code formatted as `0xCCSSPP` (matching OpenShell sysfs class strings).
    pub fn class_hex(&self) -> String {
        let combined =
            ((self.class_code as u32) << 16) | ((self.subclass as u32) << 8) | (self.prog_if as u32);
        format!("0x{:06x}", combined)
    }

    pub fn is_gpu(&self) -> bool {
        is_gpu_class_codes(self.class_code, self.subclass)
    }
}

/// Typed VFIO/PCI passthrough errors (`openshell-vfio`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfioError {
    EmptyGroup,
    InvalidBdf(String),
    DeviceNotFound(String),
    NotGpu { bdf: String, class: String },
    NotBoundToVfio { bdf: String, driver: String },
    GroupMismatch {
        bdf: String,
        expected_group: u32,
        actual_group: u32,
    },
    IommuGroupConflict {
        bdf: String,
        group: u32,
        peers: Vec<String>,
    },
}

static PCI_DEVICES: Mutex<Vec<PciInfo>> = Mutex::new(Vec::new());

/// Read a 32-bit word from PCI configuration space at `(bus, device, func, offset)`.
pub fn pci_read_u32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | (((device & 0x1F) as u32) << 11)
        | (((func & 0x07) as u32) << 8)
        | ((offset & 0xFC) as u32);
    unsafe {
        let mut addr_port = Port::<u32>::new(PCI_CONFIG_ADDRESS);
        let mut data_port = Port::<u32>::new(PCI_CONFIG_DATA);
        addr_port.write(address);
        data_port.read()
    }
}

/// Check if a PCI class/subclass is a VGA (`0x0300`) or 3D (`0x0302`) GPU controller
/// (`openshell-vfio/src/gpu.rs`).
pub fn is_gpu_class_codes(class_code: u8, subclass: u8) -> bool {
    class_code == 0x03 && (subclass == 0x00 || subclass == 0x02)
}

/// Check if a hex class string (`0x030000` or `0x0302xx`) is a GPU (`openshell-vfio/src/gpu.rs`).
pub fn is_gpu_class(class_str: &str) -> bool {
    if class_str == "0x030000" {
        return true;
    }
    if let Some(hex) = class_str.strip_prefix("0x") {
        if let Ok(val) = u32::from_str_radix(hex, 16) {
            return (val >> 8) == 0x0302;
        }
    }
    false
}

/// Validate a canonical PCI BDF string `DDDD:BB:DD.F` (`openshell-vfio/src/sysfs.rs`).
pub fn validate_bdf(bdf: &str) -> Result<(), VfioError> {
    let bytes = bdf.as_bytes();
    if bytes.len() != 12 || bytes[4] != b':' || bytes[7] != b':' || bytes[10] != b'.' {
        return Err(VfioError::InvalidBdf(bdf.to_string()));
    }
    let hex_ok = bytes[0..4].iter().all(|b| b.is_ascii_hexdigit())
        && bytes[5..7].iter().all(|b| b.is_ascii_hexdigit())
        && bytes[8..10].iter().all(|b| b.is_ascii_hexdigit())
        && (b'0'..=b'7').contains(&bytes[11]);
    if !hex_ok {
        return Err(VfioError::InvalidBdf(bdf.to_string()));
    }
    Ok(())
}

fn default_host_driver(class_code: u8, subclass: u8, vendor_id: u16) -> &'static str {
    match (class_code, subclass, vendor_id) {
        (0x06, 0x00, _) => "agpgart-intel",
        (0x06, 0x01, _) => "piix4_smbus",
        (0x01, 0x01, _) => "ata_piix",
        (0x03, _, _) => "bochs-drm",
        (0x02, 0x00, _) => "e1000",
        (_, _, VIRTIO_VENDOR_ID) => "virtio-pci",
        _ => "pci-stub",
    }
}

/// Enumerate live PCI bus 0..=3 via x86 I/O ports `0xCF8`/`0xCFC` and populate the kernel inventory.
pub fn init() -> usize {
    let mut discovered = Vec::new();
    for bus in 0u8..=3 {
        for device in 0u8..32 {
            let vendor_device = pci_read_u32(bus, device, 0, 0x00);
            let vendor_id = (vendor_device & 0xFFFF) as u16;
            if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                continue;
            }
            let header_bist = pci_read_u32(bus, device, 0, 0x0C);
            let header_type = ((header_bist >> 16) & 0xFF) as u8;
            let max_functions = if (header_type & 0x80) != 0 { 8 } else { 1 };

            for func in 0u8..max_functions {
                let vd = pci_read_u32(bus, device, func, 0x00);
                let vid = (vd & 0xFFFF) as u16;
                if vid == 0xFFFF || vid == 0x0000 {
                    continue;
                }
                let did = ((vd >> 16) & 0xFFFF) as u16;
                let class_rev = pci_read_u32(bus, device, func, 0x08);
                let prog_if = ((class_rev >> 8) & 0xFF) as u8;
                let subclass = ((class_rev >> 16) & 0xFF) as u8;
                let class_code = ((class_rev >> 24) & 0xFF) as u8;
                // Group multi-function devices on the same slot into the same IOMMU group
                let iommu_group = ((bus as u32) << 5) | (device as u32);
                let bdf = format!("0000:{:02x}:{:02x}.{}", bus, device, func);
                discovered.push(PciInfo {
                    bdf,
                    bus,
                    device,
                    function: func,
                    vendor_id: vid,
                    device_id: did,
                    class_code,
                    subclass,
                    prog_if,
                    iommu_group,
                    driver: default_host_driver(class_code, subclass, vid),
                });
            }
        }
    }
    let count = discovered.len();
    *PCI_DEVICES.lock() = discovered;
    count
}

/// Snapshot of all discovered PCI devices.
pub fn devices() -> Vec<PciInfo> {
    PCI_DEVICES.lock().clone()
}

/// RAII guard that binds a PCI device (and optional IOMMU group companions) to `vfio-pci`
/// and restores them to their host driver when dropped unless `disarm()` is called
/// (`openshell-vfio/src/pci.rs`).
pub struct PciBindGuard {
    bdf: String,
    companion_bdfs: Vec<String>,
    disarmed: bool,
}

impl PciBindGuard {
    pub fn bdf(&self) -> &str {
        &self.bdf
    }

    pub fn companion_bdfs(&self) -> &[String] {
        &self.companion_bdfs
    }

    pub fn disarm(mut self) {
        self.disarmed = true;
    }
}

impl Drop for PciBindGuard {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        let mut list = PCI_DEVICES.lock();
        for peer in &self.companion_bdfs {
            if let Some(dev) = list.iter_mut().find(|d| d.bdf == *peer) {
                dev.driver = default_host_driver(dev.class_code, dev.subclass, dev.vendor_id);
            }
        }
        if let Some(dev) = list.iter_mut().find(|d| d.bdf == self.bdf) {
            dev.driver = default_host_driver(dev.class_code, dev.subclass, dev.vendor_id);
        }
    }
}

/// Bind a GPU device and its IOMMU group companions to `vfio-pci` (`openshell-vfio/src/gpu.rs`).
pub fn prepare_gpu_for_passthrough(bdf: &str) -> Result<PciBindGuard, VfioError> {
    validate_bdf(bdf)?;
    let mut list = PCI_DEVICES.lock();
    let Some(target_idx) = list.iter().position(|d| d.bdf == bdf) else {
        return Err(VfioError::DeviceNotFound(bdf.to_string()));
    };
    let class_hex = list[target_idx].class_hex();
    if !list[target_idx].is_gpu() {
        return Err(VfioError::NotGpu {
            bdf: bdf.to_string(),
            class: class_hex,
        });
    }
    let group = list[target_idx].iommu_group;
    let mut companions = Vec::new();
    for dev in list.iter_mut() {
        if dev.iommu_group == group {
            if dev.bdf != bdf && dev.driver != "vfio-pci" {
                companions.push(dev.bdf.clone());
            }
            dev.driver = "vfio-pci";
        }
    }
    Ok(PciBindGuard {
        bdf: bdf.to_string(),
        companion_bdfs: companions,
        disarmed: false,
    })
}

/// Validate and atomically bind a complete IOMMU group to `vfio-pci`
/// (`openshell-vfio/src/pci.rs`).
pub fn prepare_pci_group_for_passthrough(bdfs: &[&str]) -> Result<PciBindGuard, VfioError> {
    let (primary, rest) = bdfs.split_first().ok_or(VfioError::EmptyGroup)?;
    for bdf in bdfs {
        validate_bdf(bdf)?;
    }
    let mut list = PCI_DEVICES.lock();
    let Some(primary_dev) = list.iter().find(|d| d.bdf == *primary) else {
        return Err(VfioError::DeviceNotFound((*primary).to_string()));
    };
    let expected_group = primary_dev.iommu_group;

    for bdf in rest {
        let Some(dev) = list.iter().find(|d| d.bdf == *bdf) else {
            return Err(VfioError::DeviceNotFound((*bdf).to_string()));
        };
        if dev.iommu_group != expected_group {
            return Err(VfioError::GroupMismatch {
                bdf: (*bdf).to_string(),
                expected_group,
                actual_group: dev.iommu_group,
            });
        }
    }

    let undeclared: Vec<String> = list
        .iter()
        .filter(|d| d.iommu_group == expected_group && !bdfs.contains(&d.bdf.as_str()))
        .map(|d| d.bdf.clone())
        .collect();
    if !undeclared.is_empty() {
        return Err(VfioError::IommuGroupConflict {
            bdf: (*primary).to_string(),
            group: expected_group,
            peers: undeclared,
        });
    }

    for dev in list.iter_mut() {
        if dev.iommu_group == expected_group {
            dev.driver = "vfio-pci";
        }
    }

    Ok(PciBindGuard {
        bdf: (*primary).to_string(),
        companion_bdfs: rest.iter().map(|s| (*s).to_string()).collect(),
        disarmed: false,
    })
}
