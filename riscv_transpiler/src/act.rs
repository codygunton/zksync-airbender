//! ACT4 (RISC-V Architecture Compliance Tests, 4th generation) integration.
//!
//! Runs self-checking ELFs produced by the ACT4 framework and returns an exit code based on
//! whether the test passed or failed. Tests signal pass/fail via the HTIF tohost mechanism:
//! writing `1` to the `tohost` symbol means pass; any other nonzero value means fail.

use object::{Object, ObjectSegment, ObjectSymbol, SegmentFlags};

use crate::ir::{preprocess_bytecode, FullMachineDecoderConfig, Instruction};
use crate::vm::{DelegationsCounters, RamPeek, RamWithRomRegion, Register, SimpleTape, State, VM};

const ROM_SECOND_WORD_BITS: usize = common_constants::rom::ROM_SECOND_WORD_BITS;
const MEMORY_SIZE: usize = 1 << 30;

/// ELF segment flag: segment is executable.
const PF_X: u32 = 1;

/// Check if an ELF segment has the executable flag set.
fn is_executable(segment: &object::Segment<'_, '_>) -> bool {
    matches!(segment.flags(), SegmentFlags::Elf { p_flags } if p_flags & PF_X != 0)
}

/// Run a self-checking ACT4 ELF and return an exit code.
///
/// Returns:
/// - `0` if the test passed (HTIF `tohost` == 1, `RVMODEL_HALT_PASS`)
/// - `1` if the test failed (HTIF `tohost` is nonzero and != 1, `RVMODEL_HALT_FAIL`)
/// - `2` if the cycle limit was exhausted before `tohost` was written
pub fn run_elf_for_act(elf_data: &[u8], max_cycles: usize) -> i32 {
    let elf = object::File::parse(elf_data).expect("act: failed to parse ELF");

    // Find the span of ALL loadable segments to size the RAM population
    let max_seg_end = elf
        .segments()
        .filter_map(|s| s.data().ok().filter(|d| !d.is_empty()).map(|_| s.address() + s.size()))
        .max()
        .unwrap_or(0) as usize;

    // Find the span of only EXECUTABLE segments for the instruction tape.
    // Data segments (.tohost, .data, .bss) must not be decoded as instructions.
    let max_exec_end = elf
        .segments()
        .filter(|s| is_executable(s))
        .filter_map(|s| s.data().ok().filter(|d| !d.is_empty()).map(|_| s.address() + s.size()))
        .max()
        .unwrap_or(0) as usize;
    let tape_words = (max_exec_end + 3) / 4;

    // Build instruction tape from executable segments only
    let mut tape_data = vec![0u32; tape_words];
    load_segments_into(&elf, &mut tape_data, true);

    // Decode executable segments into the instruction tape. Unknown opcodes (e.g. inline
    // test metadata from ACT4's failure_code.h) are decoded as Illegal instructions.
    let instructions: Vec<Instruction> =
        preprocess_bytecode::<FullMachineDecoderConfig>(&tape_data);
    let tape = SimpleTape::new(&instructions);

    // Allocate 1 GB RAM backing and populate from ALL segments (code + data)
    let all_words = (max_seg_end + 3) / 4;
    let ram_words = MEMORY_SIZE / core::mem::size_of::<u32>();
    let mut backing = vec![Register { value: 0, timestamp: 0 }; ram_words];
    // Load all segments (including data) into RAM
    let mut all_data = vec![0u32; all_words];
    load_segments_into(&elf, &mut all_data, false);
    for (i, &word) in all_data.iter().enumerate() {
        backing[i].value = word;
    }
    let mut ram = RamWithRomRegion::<ROM_SECOND_WORD_BITS> { backing };

    let entry_point = elf.entry() as u32;
    let tohost_addr = elf
        .symbols()
        .find(|s| s.name() == Ok("tohost"))
        .map(|s| s.address() as u32)
        .expect("act: ELF has no 'tohost' symbol — check linker script");

    let mut state = State::initial_with_counters(DelegationsCounters::default());
    state.pc = entry_point;

    const POLL_CHUNK: usize = 100_000;
    let mut remaining = max_cycles;

    // Wrap the execution loop in catch_unwind to handle panics from unsupported
    // instructions (e.g., DIV, FENCE) that Airbender encounters during execution.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            let chunk = remaining.min(POLL_CHUNK);
            VM::<DelegationsCounters>::run_basic_unrolled(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                chunk,
                &mut (),
            );
            remaining = remaining.saturating_sub(chunk);

            let tohost_val = ram.peek_word(tohost_addr);
            if tohost_val == 1 {
                return 0; // RVMODEL_HALT_PASS
            } else if tohost_val != 0 {
                return 1; // RVMODEL_HALT_FAIL
            }
            if remaining == 0 {
                eprintln!("act: cycle limit ({max_cycles}) exhausted without tohost signal");
                return 2;
            }
        }
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            eprintln!("act: VM panicked (unsupported instruction encountered during execution)");
            1
        }
    }
}

/// Load ELF PT_LOAD segments into a flat word array.
///
/// If `exec_only` is true, only executable segments are loaded (for the instruction tape).
/// If false, all loadable segments are loaded (for RAM).
///
/// The array must be large enough to hold all relevant segments. Words are stored at `addr / 4`.
fn load_segments_into(elf: &object::File<'_>, words: &mut [u32], exec_only: bool) {
    for segment in elf.segments() {
        if exec_only && !is_executable(&segment) {
            continue;
        }
        let Ok(data) = segment.data() else { continue };
        if data.is_empty() {
            continue;
        }
        let addr = segment.address() as usize;
        let word_start = addr / 4;

        let padded_len = (data.len() + 3) / 4 * 4;
        let mut padded = data.to_vec();
        padded.resize(padded_len, 0);

        for (i, chunk) in padded.chunks_exact(4).enumerate() {
            let idx = word_start + i;
            if idx < words.len() {
                words[idx] = u32::from_le_bytes(chunk.try_into().unwrap());
            }
        }
    }
}
