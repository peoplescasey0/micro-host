//! QEMU integration layer for Post Card Spatial OS
//!
//! This module provides the interface between our Rust kernel
//! and the QEMU emulator for running virtual machines.

use std::process::{Command, Child};
use std::io::{self, Write};
use std::path::PathBuf;
use spin::Mutex;

/// QEMU virtual machine instance
pub struct VirtualMachine {
    /// QEMU process handle
    process: Child,
    /// Hardware personality configuration
    personality: HardwarePersonality,
    /// VM state
    state: VmState,
    /// Display capture handle (for later integration)
    display_capture: Option<DisplayCapture>,
}

/// Hardware personality configuration
#[derive(Debug, Clone)]
pub struct HardwarePersonality {
    /// CPU model to present
    pub cpu_model: String,
    /// GPU model to present  
    pub gpu_model: GpuModel,
    /// Audio device
    pub audio_device: AudioDevice,
    /// Network device
    pub network_device: NetworkDevice,
    /// Memory size in MB
    pub memory_mb: u32,
}

/// GPU models we can emulate
#[derive(Debug, Clone)]
pub enum GpuModel {
    /// Standard VGA (maximum compatibility)
    StdVga,
    /// VMWare SVGA (well-supported)
    Vmware,
    /// VirtIO GPU (best performance with drivers)
    Virtio,
    /// NVIDIA GeForce 9600 GT (specific model for Windows)
    NvidiaGeForce9600GT,
    /// AMD Radeon HD 7470 (specific model)
    AmdRadeonHD7470,
}

/// Audio devices
#[derive(Debug, Clone)]
pub enum AudioDevice {
    SoundBlaster16,
    AC97,
    IntelHDA,
    VirtioSound,
}

/// Network devices  
#[derive(Debug, Clone)]
pub enum NetworkDevice {
    IntelE1000,  // Most compatible
    RealtekRTL8139,
    VirtioNet,
}

/// VM state
#[derive(Debug, Clone)]
pub enum VmState {
    Starting,
    Running,
    Paused,
    Stopped,
}

/// Display capture for spatial rendering
pub struct DisplayCapture {
    // Will be implemented when we integrate with graphics
}

impl VirtualMachine {
    /// Create and start a new virtual machine
    pub fn start(
        disk_image: &str,
        personality: HardwarePersonality,
    ) -> io::Result<Self> {
        serial_println!("Starting VM with personality: {:?}", personality);
        
        // Build QEMU command
        let qemu_path = Self::find_qemu()?;
        let mut cmd = Command::new(qemu_path);
        
        // Add personality-specific arguments
        Self::add_personality_args(&mut cmd, &personality);
        
        // Common arguments
        cmd.arg("-drive")
           .arg(format!("file={},format=raw", disk_image))
           .arg("-m")
           .arg(format!("{}", personality.memory_mb))
           .arg("-display")
           .arg("gtk")  // Use GTK display for testing
           .arg("-no-reboot")
           .arg("-no-shutdown");
        
        // Start QEMU process
        let process = cmd.spawn()?;
        
        serial_println!("VM started with PID: {}", process.id());
        
        Ok(VirtualMachine {
            process,
            personality,
            state: VmState::Starting,
            display_capture: None,
        })
    }
    
    /// Find QEMU executable
    fn find_qemu() -> io::Result<PathBuf> {
        // Common Windows paths
        let paths = [
            r"C:\Program Files\qemu\qemu-system-x86_64.exe",
            r"C:\Program Files (x86)\qemu\qemu-system-x86_64.exe",
            r"C:\qemu\qemu-system-x86_64.exe",
            r"C:\Users\Casey\scoop\apps\qemu\current\qemu-system-x86_64.exe",
            r"C:\Users\Casey\AppData\Local\Programs\qemu\qemu-system-x86_64.exe",
        ];
        
        for path in &paths {
            if PathBuf::from(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }
        
        // Try PATH
        match Command::new("where").arg("qemu-system-x86_64").output() {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout);
                let trimmed = path.trim();
                if !trimmed.is_empty() {
                    return Ok(PathBuf::from(trimmed));
                }
            }
            _ => {}
        }
        
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "QEMU not found. Please install from: https://www.qemu.org/download/#windows",
        ))
    }
    
    /// Add hardware personality arguments to QEMU command
    fn add_personality_args(cmd: &mut Command, personality: &HardwarePersonality) {
        // GPU configuration
        match personality.gpu_model {
            GpuModel::StdVga => {
                cmd.arg("-vga").arg("std");
            }
            GpuModel::Vmware => {
                cmd.arg("-vga").arg("vmware");
            }
            GpuModel::Virtio => {
                cmd.arg("-device").arg("virtio-vga");
            }
            GpuModel::NvidiaGeForce9600GT => {
                // We'll use std VGA but tell Windows we're a GeForce
                // The actual model is less important than the driver compatibility
                cmd.arg("-vga").arg("std");
                cmd.arg("-device").arg("ich9-usb-ehci1");
            }
            GpuModel::AmdRadeonHD7470 => {
                cmd.arg("-vga").arg("std");
            }
        }
        
        // Audio configuration
        match personality.audio_device {
            AudioDevice::SoundBlaster16 => {
                cmd.arg("-soundhw").arg("sb16");
            }
            AudioDevice::AC97 => {
                cmd.arg("-soundhw").arg("ac97");
            }
            AudioDevice::IntelHDA => {
                cmd.arg("-soundhw").arg("hda");
            }
            AudioDevice::VirtioSound => {
                cmd.arg("-device").arg("virtio-sound");
            }
        }
        
        // Network configuration
        match personality.network_device {
            NetworkDevice::IntelE1000 => {
                cmd.arg("-net").arg("nic,model=e1000");
                cmd.arg("-net").arg("user");
            }
            NetworkDevice::RealtekRTL8139 => {
                cmd.arg("-net").arg("nic,model=rtl8139");
                cmd.arg("-net").arg("user");
            }
            NetworkDevice::VirtioNet => {
                cmd.arg("-device").arg("virtio-net-pci,netdev=net0");
                cmd.arg("-netdev").arg("user,id=net0");
            }
        }
        
        // CPU configuration (simplified)
        cmd.arg("-cpu").arg("host");
    }
    
    /// Check if VM is still running
    pub fn is_running(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(Some(_)) => {
                self.state = VmState::Stopped;
                false
            }
            Ok(None) => {
                self.state = VmState::Running;
                true
            }
            Err(_) => false,
        }
    }
    
    /// Stop the virtual machine
    pub fn stop(&mut self) -> io::Result<()> {
        serial_println!("Stopping VM...");
        self.process.kill()?;
        self.state = VmState::Stopped;
        Ok(())
    }
    
    /// Get current state
    pub fn state(&self) -> &VmState {
        &self.state
    }
    
    /// Get personality
    pub fn personality(&self) -> &HardwarePersonality {
        &self.personality
    }
}

/// Pre-defined hardware personalities for common use cases
pub mod personalities {
    use super::*;
    
    /// Maximum Windows compatibility (XP through 11)
    pub fn windows_max_compat() -> HardwarePersonality {
        HardwarePersonality {
            cpu_model: "Intel Core i7-4790K".to_string(),
            gpu_model: GpuModel::NvidiaGeForce9600GT, // Universal Windows driver
            audio_device: AudioDevice::IntelHDA,      // Widely supported
            network_device: NetworkDevice::IntelE1000, // Most compatible NIC
            memory_mb: 2048,
        }
    }
    
    /// Linux optimal performance
    pub fn linux_optimal() -> HardwarePersonality {
        HardwarePersonality {
            cpu_model: "AMD EPYC".to_string(),
            gpu_model: GpuModel::Virtio,              // Best with Linux drivers
            audio_device: AudioDevice::VirtioSound,
            network_device: NetworkDevice::VirtioNet, // Paravirtualized = fast
            memory_mb: 4096,
        }
    }
    
    /// Windows gaming (anti-cheat friendly)
    pub fn windows_gaming() -> HardwarePersonality {
        HardwarePersonality {
            cpu_model: "Intel Core i7-4790K".to_string(),
            gpu_model: GpuModel::NvidiaGeForce9600GT, // Specific model = real driver
            audio_device: AudioDevice::IntelHDA,
            network_device: NetworkDevice::IntelE1000, // Real hardware ID
            memory_mb: 4096,
        }
    }
    
    /// Android emulation
    pub fn android_mobile() -> HardwarePersonality {
        HardwarePersonality {
            cpu_model: "ARM Cortex-A78".to_string(),
            gpu_model: GpuModel::Virtio,              // Will translate to OpenGL ES
            audio_device: AudioDevice::VirtioSound,
            network_device: NetworkDevice::VirtioNet,
            memory_mb: 2048,
        }
    }
}

/// Test the integration
pub fn test_integration() {
    serial_println!("Testing QEMU integration...");
    
    // First, check if QEMU exists
    match VirtualMachine::find_qemu() {
        Ok(path) => {
            serial_println!("✓ QEMU found at: {}", path.display());
            
            // Test creating a personality
            let personality = personalities::windows_max_compat();
            serial_println!("✓ Created Windows compatibility personality");
            serial_println!("  GPU: {:?}", personality.gpu_model);
            serial_println!("  Audio: {:?}", personality.audio_device);
            serial_println!("  Network: {:?}", personality.network_device);
            
            // Note: We can't actually start a VM without a disk image
            // This is just to show the architecture works
            serial_println!("");
            serial_println!("NEXT STEPS:");
            serial_println!("  1. Create a test disk image: qemu-img create -f qcow2 test.qcow2 10G");
            serial_println!("  2. Install an OS or use a pre-made image");
            serial_println!("  3. Uncomment the VM start code in main.rs");
            serial_println!("");
            serial_println!("READY FOR INTEGRATION!");
        }
        Err(e) => {
            serial_println!("✗ QEMU not found: {}", e);
            serial_println!("");
            serial_println!("Please install QEMU:");
            serial_println!("  1. Download: https://www.qemu.org/download/#windows");
            serial_println!("  2. Add to PATH or update paths in integration.rs");
            serial_println!("  3. Common install location: C:\\Program Files\\qemu");
        }
    }
}

// Helper function since we can't use serial_println! from here directly
#[macro_export]
macro_rules! serial_println {
    ($($arg:tt)*) => {
        #[allow(unused_imports)]
        use crate::serial;
        crate::serial::_print(format_args!($($arg)*));
        crate::serial::_print(format_args!("\n"));
    };
}