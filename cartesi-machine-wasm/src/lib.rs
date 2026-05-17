//! Cartesi Machine WASM bindings
//!
//! Compiles the Cartesi Machine RISC-V emulator to WebAssembly,
//! enabling deterministic Linux execution in the browser.

use wasm_bindgen::prelude::*;
use cartesi_machine::{
    config::{machine::MachineConfig, runtime::RuntimeConfig},
    Machine,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// Constants (wasm-bindgen doesn't support pub const; use functions)
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub fn break_reason_failed() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_FAILED as u32 }
#[wasm_bindgen]
pub fn break_reason_halted() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_HALTED as u32 }
#[wasm_bindgen]
pub fn break_reason_yielded_manually() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_YIELDED_MANUALLY as u32 }
#[wasm_bindgen]
pub fn break_reason_yielded_automatically() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_YIELDED_AUTOMATICALLY as u32 }
#[wasm_bindgen]
pub fn break_reason_yielded_softly() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_YIELDED_SOFTLY as u32 }
#[wasm_bindgen]
pub fn break_reason_reached_target() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_REACHED_TARGET_MCYCLE as u32 }
#[wasm_bindgen]
pub fn break_reason_console_output() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_CONSOLE_OUTPUT as u32 }
#[wasm_bindgen]
pub fn break_reason_console_input() -> u32 { cartesi_machine_sys::CM_BREAK_REASON_CONSOLE_INPUT as u32 }

/// Get a human-readable name for a break reason code.
#[wasm_bindgen]
pub fn break_reason_name(reason: u32) -> String {
    match reason as i32 {
        x if x == cartesi_machine_sys::CM_BREAK_REASON_FAILED as i32 => "failed".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_HALTED as i32 => "halted".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_YIELDED_MANUALLY as i32 => "yielded_manually".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_YIELDED_AUTOMATICALLY as i32 => "yielded_automatically".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_YIELDED_SOFTLY as i32 => "yielded_softly".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_REACHED_TARGET_MCYCLE as i32 => "reached_target".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_CONSOLE_OUTPUT as i32 => "console_output".into(),
        x if x == cartesi_machine_sys::CM_BREAK_REASON_CONSOLE_INPUT as i32 => "console_input".into(),
        _ => format!("unknown({})", reason),
    }
}

// Register indices
#[wasm_bindgen] pub fn reg_pc() -> u32 { cartesi_machine_sys::CM_REG_PC as u32 }
#[wasm_bindgen] pub fn reg_mcycle() -> u32 { cartesi_machine_sys::CM_REG_MCYCLE as u32 }
#[wasm_bindgen] pub fn reg_x10() -> u32 { cartesi_machine_sys::CM_REG_X10 as u32 }
#[wasm_bindgen] pub fn reg_x11() -> u32 { cartesi_machine_sys::CM_REG_X11 as u32 }
#[wasm_bindgen] pub fn reg_x12() -> u32 { cartesi_machine_sys::CM_REG_X12 as u32 }
#[wasm_bindgen] pub fn reg_x13() -> u32 { cartesi_machine_sys::CM_REG_X13 as u32 }
#[wasm_bindgen] pub fn reg_x14() -> u32 { cartesi_machine_sys::CM_REG_X14 as u32 }
#[wasm_bindgen] pub fn reg_x15() -> u32 { cartesi_machine_sys::CM_REG_X15 as u32 }

// ---------------------------------------------------------------------------
// WasmMachine
// ---------------------------------------------------------------------------

/// A Cartesi Machine running inside WebAssembly.
#[wasm_bindgen]
pub struct WasmMachine {
    inner: Machine,
}

#[wasm_bindgen]
impl WasmMachine {
    /// Create a new Cartesi Machine from JSON configuration strings.
    ///
    /// Example config (JSON):
    /// ```json
    /// {
    ///   "ram": {"length": 268435456},
    ///   "flash_drive": [{"length": 402653184}],
    ///   "dtb": {"bootargs": "console=hvc0 root=/dev/pmem0 rw", "entrypoint": "exec ash -l"},
    ///   "processor": {"iunrep": 1}
    /// }
    /// ```
    pub fn create(config_json: &str, runtime_json: &str) -> Result<WasmMachine, JsValue> {
        let config: MachineConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid config JSON: {}", e)))?;
        let runtime: RuntimeConfig = serde_json::from_str(runtime_json)
            .unwrap_or_default();

        let machine = Machine::create(&config, &runtime)
            .map_err(|e| JsValue::from_str(&format!("Failed to create machine: {}", e)))?;

        Ok(WasmMachine { inner: machine })
    }

    /// Run the machine until mcycle reaches mcycle_end or it yields/halts.
    /// Returns the break reason as a u32.
    pub fn run(&mut self, mcycle_end: u64) -> Result<u32, JsValue> {
        self.inner
            .run(mcycle_end)
            .map(|r| r as u32)
            .map_err(|e| JsValue::from_str(&format!("Run failed: {}", e)))
    }

    /// Read physical memory. Returns a Uint8Array.
    pub fn read_memory(&mut self, address: u64, length: u64) -> Result<Vec<u8>, JsValue> {
        self.inner
            .read_memory(address, length)
            .map_err(|e| JsValue::from_str(&format!("Read memory failed: {}", e)))
    }

    /// Write data to physical memory.
    pub fn write_memory(&mut self, address: u64, data: &[u8]) -> Result<(), JsValue> {
        self.inner
            .write_memory(address, data)
            .map_err(|e| JsValue::from_str(&format!("Write memory failed: {}", e)))
    }

    /// Read a RISC-V register (see reg_* functions for indices).
    pub fn read_register(&mut self, reg: u32) -> Result<u64, JsValue> {
        self.inner
            .read_reg(reg)
            .map_err(|e| JsValue::from_str(&format!("Read register failed: {}", e)))
    }

    /// Write a RISC-V register.
    pub fn write_register(&mut self, reg: u32, value: u64) -> Result<(), JsValue> {
        self.inner
            .write_reg(reg, value)
            .map_err(|e| JsValue::from_str(&format!("Write register failed: {}", e)))
    }

    /// Get the machine cycle counter (mcycle CSR).
    pub fn get_mcycle(&mut self) -> Result<u64, JsValue> {
        self.inner
            .mcycle()
            .map_err(|e| JsValue::from_str(&format!("Get mcycle failed: {}", e)))
    }

    /// Get the emulator version string (e.g., "0.20.0").
    pub fn get_version() -> String {
        cartesi_machine::format_emulator_version(cartesi_machine::EXPECTED_EMULATOR_VERSION)
    }

    /// Get the machine's root hash as a hex string (64 chars).
    pub fn get_root_hash(&mut self) -> Result<String, JsValue> {
        self.inner
            .root_hash()
            .map(|h| hex::encode(h))
            .map_err(|e| JsValue::from_str(&format!("Get root hash failed: {}", e)))
    }

    /// Replace a memory range. Use for loading kernel/rootfs images.
    /// Pass null/undefined for image_path to create a zero-filled range.
    pub fn replace_memory_range(
        &mut self,
        start: u64,
        length: u64,
        shared: bool,
        image_path: Option<String>,
    ) -> Result<(), JsValue> {
        let path = image_path.as_ref().map(|p| std::path::Path::new(p.as_str()));
        self.inner
            .replace_memory_range(start, length, shared, path)
            .map_err(|e| JsValue::from_str(&format!("Replace memory range failed: {}", e)))
    }
}
