//! Generic transpiler-based binary runner.
//!
//! Loads a flat binary into the VM at a given entry point, runs it through the transpiler
//! execution path (`preprocess_bytecode` + `VM::run_basic_unrolled`), and optionally polls
//! an HTIF `tohost` address to detect program termination.
//!
//! This is the same execution path used by the prover, making it suitable for compliance
//! testing and general simulation.

use crate::ir::{preprocess_bytecode, FullMachineDecoderConfig, Instruction};
use crate::vm::{DelegationsCounters, RamPeek, RamWithRomRegion, Register, SimpleTape, State, VM};

const ROM_SECOND_WORD_BITS: usize = common_constants::rom::ROM_SECOND_WORD_BITS;
const MEMORY_SIZE: usize = 1 << 30;
const POLL_CHUNK: usize = 100_000;

/// Result of running a binary through the transpiler VM.
pub struct RunResult {
    /// Final register values x0..x31 (index i = register xi).
    pub registers: [u32; 32],
    /// The value written to the tohost address when the program terminated.
    /// `Some(v)` if `tohost_addr` was provided and the program wrote a nonzero value.
    /// `None` if `tohost_addr` was not provided, or if the cycle limit was exhausted
    /// before tohost was written.
    pub tohost_value: Option<u32>,
    /// `true` iff the cycle limit was exhausted before tohost fired.
    /// Only meaningful when `tohost_addr` was `Some`.
    pub timed_out: bool,
}

/// Run a flat binary through the transpiler VM.
///
/// The binary is placed at `entry_point` in the address space (same address as initial PC).
/// This matches the output of `riscv64-unknown-elf-objcopy -O binary`, where the file
/// content begins at the lowest load VMA of the ELF.
///
/// # Arguments
/// * `binary` - Raw bytes of the flat binary.
/// * `entry_point` - Load address and initial program counter.
/// * `max_cycles` - Hard cycle ceiling. Execution stops after this many cycles.
/// * `tohost_addr` - If `Some`, poll this word address every `POLL_CHUNK` cycles.
///   Returns when the value becomes nonzero. If `None`, runs for `max_cycles` and returns.
pub fn run_binary(
    binary: &[u8],
    entry_point: u32,
    max_cycles: usize,
    tohost_addr: Option<u32>,
) -> RunResult {
    let binary_words = bytes_to_words(binary);
    let entry_offset = (entry_point / 4) as usize;
    let total_words = entry_offset + binary_words.len();

    // Instruction tape: pad to entry_point with zeros, then place binary words
    let mut padded_instructions = vec![0u32; total_words];
    padded_instructions[entry_offset..].copy_from_slice(&binary_words);

    let instructions: Vec<Instruction> =
        preprocess_bytecode::<FullMachineDecoderConfig>(&padded_instructions);
    let tape = SimpleTape::new(&instructions);

    // RAM: 1 GB backing, binary placed at entry_offset
    let ram_words = MEMORY_SIZE / core::mem::size_of::<u32>();
    let mut backing = vec![Register { value: 0, timestamp: 0 }; ram_words];
    for (i, &word) in binary_words.iter().enumerate() {
        backing[entry_offset + i].value = word;
    }
    let mut ram = RamWithRomRegion::<ROM_SECOND_WORD_BITS> { backing };

    let mut state = State::initial_with_counters(DelegationsCounters::default());
    state.pc = entry_point;

    if let Some(tohost) = tohost_addr {
        let mut remaining = max_cycles;
        while remaining > 0 {
            let chunk = remaining.min(POLL_CHUNK);
            VM::<DelegationsCounters>::run_basic_unrolled(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                chunk,
                &mut (),
            );
            remaining -= chunk;

            let tohost_val = ram.peek_word(tohost);
            if tohost_val != 0 {
                return RunResult {
                    registers: state.registers.map(|r| r.value),
                    tohost_value: Some(tohost_val),
                    timed_out: false,
                };
            }
        }
        // Cycle limit exhausted
        RunResult {
            registers: state.registers.map(|r| r.value),
            tohost_value: None,
            timed_out: true,
        }
    } else {
        VM::<DelegationsCounters>::run_basic_unrolled(
            &mut state,
            &mut ram,
            &mut (),
            &tape,
            max_cycles,
            &mut (),
        );
        RunResult {
            registers: state.registers.map(|r| r.value),
            tohost_value: None,
            timed_out: false,
        }
    }
}

fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    let padded_len = (bytes.len() + 3) / 4 * 4;
    let mut padded = bytes.to_vec();
    padded.resize(padded_len, 0);

    padded
        .as_chunks::<4>()
        .0
        .iter()
        .map(|el| u32::from_le_bytes(*el))
        .collect()
}
