use crate::abstractions::memory::MemorySource;
use crate::abstractions::memory::VectorMemoryImpl;
use crate::abstractions::non_determinism::NonDeterminismCSRSource;
use crate::cycle::state::RiscV32MachineV1;
use crate::cycle::MachineConfig;
use crate::mmu::NoMMU;
use crate::setup::BaselineWithND;
use crate::sim::{RiscV32MachineSetup, Simulator, SimulatorConfig};
use crate::signature_extraction;
use std::path::Path;

/// Run a simulation and extract RISCOF test signatures to a file
pub fn run_with_signature_extraction<S, C>(
    config: SimulatorConfig,
    non_determinism_source: S,
    elf_data: &[u8],
    signature_path: &Path,
) -> Result<[u32; 16], signature_extraction::SignatureExtractionError>
where
    S: NonDeterminismCSRSource<VectorMemoryImpl>,
    C: MachineConfig,
{
    let setup = BaselineWithND::<_, C>::new(non_determinism_source);
    let mut sim = Simulator::<_, C>::new(config, setup);
    
    // Run the simulation
    let result = sim.run(|_, _| {}, |_, _| {});
    
    // Get registers for return value
    let mut registers = [0u32; 16];
    registers.copy_from_slice(&result.state.registers[10..26]);
    
    // Try to extract signatures if ELF data contains the bounds
    match signature_extraction::find_signature_bounds(elf_data) {
        Ok(mut bounds) => {
            // Access the memory from the machine
            // Note: This requires the machine field to be accessible
            let memory = &sim.machine.memory_source;
            
            // Adjust bounds for the actual memory layout
            // The binary is loaded at 0x0, but ELF addresses are at 0x10000000+
            // So we need to mask off the high bits to get the actual offset
            let adjusted_begin = bounds.begin_address & 0xFFFFFF;
            let adjusted_end = bounds.end_address & 0xFFFFFF;
            
            println!("Original bounds: 0x{:x} - 0x{:x}", bounds.begin_address, bounds.end_address);
            println!("Adjusted bounds: 0x{:x} - 0x{:x}", adjusted_begin, adjusted_end);
            
            bounds.begin_address = adjusted_begin;
            bounds.end_address = adjusted_end;
            
            // Collect signatures from memory
            let signatures = signature_extraction::collect_signatures(memory, bounds)?;
            
            // Write to file
            signature_extraction::write_signatures(&signatures, signature_path)?;
            
            println!("Extracted {} signature words from 0x{:x} to 0x{:x}", 
                     signatures.len(), bounds.begin_address, bounds.end_address);
        }
        Err(signature_extraction::SignatureExtractionError::SymbolNotFound(_)) => {
            // No signature bounds found - create empty file for RISCOF compatibility
            std::fs::File::create(signature_path)
                .map_err(|e| signature_extraction::SignatureExtractionError::FileIo(e))?;
            println!("No signature bounds found in ELF, created empty signature file");
        }
        Err(e) => {
            // Other errors - still create empty file but warn
            eprintln!("Warning: Signature extraction failed: {}", e);
            std::fs::File::create(signature_path)
                .map_err(|e| signature_extraction::SignatureExtractionError::FileIo(e))?;
        }
    }
    
    Ok(registers)
}