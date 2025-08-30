use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use elf::endian::AnyEndian;
use elf::ElfBytes;

use crate::abstractions::memory::{AccessType, MemorySource, VectorMemoryImpl};
use crate::cycle::status_registers::TrapReason;

/// Error types for signature extraction operations.
#[derive(Debug)]
pub enum SignatureExtractionError {
    /// ELF parsing failed
    ElfParsing(String),
    /// Required symbol not found
    SymbolNotFound(String),
    /// Memory access failed
    MemoryAccess(String),
    /// File I/O error
    FileIo(std::io::Error),
    /// Invalid memory range
    InvalidRange(String),
}

impl std::fmt::Display for SignatureExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureExtractionError::ElfParsing(msg) => write!(f, "ELF parsing error: {}", msg),
            SignatureExtractionError::SymbolNotFound(symbol) => write!(f, "Symbol not found: {}", symbol),
            SignatureExtractionError::MemoryAccess(msg) => write!(f, "Memory access error: {}", msg),
            SignatureExtractionError::FileIo(err) => write!(f, "File I/O error: {}", err),
            SignatureExtractionError::InvalidRange(msg) => write!(f, "Invalid range: {}", msg),
        }
    }
}

impl std::error::Error for SignatureExtractionError {}

impl From<std::io::Error> for SignatureExtractionError {
    fn from(err: std::io::Error) -> Self {
        SignatureExtractionError::FileIo(err)
    }
}

/// Signature bounds representing the start and end addresses of the signature region
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureBounds {
    pub begin_address: u64,
    pub end_address: u64,
}

impl SignatureBounds {
    /// Create a new SignatureBounds instance
    pub fn new(begin_address: u64, end_address: u64) -> Result<Self, SignatureExtractionError> {
        if begin_address >= end_address {
            return Err(SignatureExtractionError::InvalidRange(
                format!("Begin address (0x{:x}) must be less than end address (0x{:x})", 
                        begin_address, end_address)
            ));
        }
        
        if begin_address % 4 != 0 || end_address % 4 != 0 {
            return Err(SignatureExtractionError::InvalidRange(
                format!("Addresses must be 4-byte aligned: begin=0x{:x}, end=0x{:x}", 
                        begin_address, end_address)
            ));
        }
        
        Ok(Self { begin_address, end_address })
    }
    
    /// Get the size of the signature region in bytes
    pub fn size_bytes(&self) -> u64 {
        self.end_address - self.begin_address
    }
    
    /// Get the number of 4-byte words in the signature region
    pub fn word_count(&self) -> u64 {
        self.size_bytes() / 4
    }
}

/// Find the signature bounds from ELF data by locating begin_signature and end_signature symbols
///
/// # Arguments
///
/// * `elf_data` - The ELF file data as a byte slice
///
/// # Returns
///
/// Returns the signature bounds containing the start and end addresses
///
/// # Errors
///
/// Returns an error if:
/// - ELF parsing fails
/// - begin_signature or end_signature symbols are not found
/// - Symbol addresses are invalid
pub fn find_signature_bounds(elf_data: &[u8]) -> Result<SignatureBounds, SignatureExtractionError> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf_data)
        .map_err(|e| SignatureExtractionError::ElfParsing(format!("Failed to parse ELF: {}", e)))?;
    
    // Find common ELF data sections
    let common = elf
        .find_common_data()
        .map_err(|e| SignatureExtractionError::ElfParsing(format!("Failed to find common data: {}", e)))?;
    
    // Try to get symbol table and string table from different sources
    let (symtab, strtab) = if let (Some(symtab), Some(strtab)) = (common.symtab, common.symtab_strs) {
        (symtab, strtab)
    } else if let (Some(dynsyms), Some(dynsyms_strs)) = (common.dynsyms, common.dynsyms_strs) {
        (dynsyms, dynsyms_strs)
    } else {
        return Err(SignatureExtractionError::ElfParsing(
            "No symbol table found in ELF file".to_string()
        ));
    };
    
    // Create a map of symbol names to addresses
    let mut symbols: HashMap<String, u64> = HashMap::new();
    
    // Iterate through symbols and build the lookup map
    for symbol in symtab.iter() {
        if let Ok(name) = strtab.get(symbol.st_name as usize) {
            symbols.insert(name.to_string(), symbol.st_value);
        }
    }
    
    // Find the required symbols
    let begin_address = symbols
        .get("begin_signature")
        .copied()
        .ok_or_else(|| SignatureExtractionError::SymbolNotFound("begin_signature".to_string()))?;
    
    let end_address = symbols
        .get("end_signature")
        .copied()
        .ok_or_else(|| SignatureExtractionError::SymbolNotFound("end_signature".to_string()))?;
    
    SignatureBounds::new(begin_address, end_address)
}

/// Collect signatures from memory by reading 4-byte words from the signature region
///
/// # Arguments
///
/// * `memory` - The memory implementation to read from
/// * `bounds` - The signature bounds defining the region to read
///
/// # Returns
///
/// Returns a vector of 32-bit words representing the signature data
///
/// # Errors
///
/// Returns an error if:
/// - Memory access fails
/// - Memory trap occurs during reading
pub fn collect_signatures(
    memory: &VectorMemoryImpl,
    bounds: SignatureBounds,
) -> Result<Vec<u32>, SignatureExtractionError> {
    let word_count = bounds.word_count() as usize;
    let mut signatures = Vec::with_capacity(word_count);
    let mut current_address = bounds.begin_address;
    
    while current_address < bounds.end_address {
        let mut trap = TrapReason::NoTrap;
        let word = memory.get(current_address, AccessType::MemLoad, &mut trap);
        
        // Check if a trap occurred during memory access
        if trap != TrapReason::NoTrap {
            return Err(SignatureExtractionError::MemoryAccess(
                format!("Memory trap occurred at address 0x{:x}: {:?}", current_address, trap)
            ));
        }
        
        signatures.push(word);
        current_address += 4;
    }
    
    Ok(signatures)
}

/// Write signatures to a file in the specified format (8-character lowercase hex, one per line)
///
/// # Arguments
///
/// * `signatures` - The vector of 32-bit signature words to write
/// * `output_path` - The path to the output file
///
/// # Returns
///
/// Returns `Ok(())` if successful
///
/// # Errors
///
/// Returns an error if file I/O operations fail
pub fn write_signatures<P: AsRef<Path>>(
    signatures: &[u32],
    output_path: P,
) -> Result<(), SignatureExtractionError> {
    let mut file = File::create(output_path)?;
    
    for &signature in signatures {
        writeln!(file, "{:08x}", signature)?;
    }
    
    Ok(())
}

/// Extract signatures from ELF data and write them to a file
///
/// This is a convenience function that combines all the signature extraction steps:
/// 1. Find signature bounds in ELF data
/// 2. Collect signatures from memory
/// 3. Write signatures to file
///
/// # Arguments
///
/// * `elf_data` - The ELF file data as a byte slice
/// * `memory` - The memory implementation to read from
/// * `output_path` - The path to the output file
///
/// # Returns
///
/// Returns the signature bounds that were used for extraction
///
/// # Errors
///
/// Returns an error if any step fails
pub fn extract_signatures_to_file<P: AsRef<Path>>(
    elf_data: &[u8],
    memory: &VectorMemoryImpl,
    output_path: P,
) -> Result<SignatureBounds, SignatureExtractionError> {
    let bounds = find_signature_bounds(elf_data)?;
    let signatures = collect_signatures(memory, bounds)?;
    write_signatures(&signatures, output_path)?;
    Ok(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abstractions::memory::VectorMemoryImpl;
    use std::fs;
    use std::io::Read;

    #[test]
    fn test_signature_bounds_creation() {
        // Valid bounds
        let bounds = SignatureBounds::new(0x1000, 0x2000).unwrap();
        assert_eq!(bounds.begin_address, 0x1000);
        assert_eq!(bounds.end_address, 0x2000);
        assert_eq!(bounds.size_bytes(), 0x1000);
        assert_eq!(bounds.word_count(), 0x400);

        // Invalid bounds - begin >= end
        assert!(SignatureBounds::new(0x2000, 0x1000).is_err());
        assert!(SignatureBounds::new(0x1000, 0x1000).is_err());

        // Invalid bounds - not 4-byte aligned
        assert!(SignatureBounds::new(0x1001, 0x2000).is_err());
        assert!(SignatureBounds::new(0x1000, 0x2001).is_err());
    }

    #[test]
    fn test_collect_signatures() {
        // Create a test memory with known values
        let mut memory = VectorMemoryImpl::new_for_byte_size(0x4000);
        
        // Populate memory with test data at addresses 0x1000-0x1010
        memory.populate(0x1000, 0xdeadbeef);
        memory.populate(0x1004, 0xcafebabe);
        memory.populate(0x1008, 0x12345678);
        memory.populate(0x100c, 0x87654321);
        
        let bounds = SignatureBounds::new(0x1000, 0x1010).unwrap();
        let signatures = collect_signatures(&memory, bounds).unwrap();
        
        assert_eq!(signatures.len(), 4);
        assert_eq!(signatures[0], 0xdeadbeef);
        assert_eq!(signatures[1], 0xcafebabe);
        assert_eq!(signatures[2], 0x12345678);
        assert_eq!(signatures[3], 0x87654321);
    }

    #[test]
    fn test_write_signatures() {
        let signatures = vec![0xdeadbeef, 0xcafebabe, 0x12345678];
        let temp_file = "/tmp/test_signatures.txt";
        
        write_signatures(&signatures, temp_file).unwrap();
        
        let mut contents = String::new();
        let mut file = fs::File::open(temp_file).unwrap();
        file.read_to_string(&mut contents).unwrap();
        
        let expected = "deadbeef\ncafebabe\n12345678\n";
        assert_eq!(contents, expected);
        
        // Clean up
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_collect_signatures_out_of_bounds() {
        // Create a small memory that doesn't cover the requested range
        let memory = VectorMemoryImpl::new_for_byte_size(0x1000);
        
        // Try to read beyond memory bounds
        let bounds = SignatureBounds::new(0x2000, 0x2010).unwrap();
        let result = collect_signatures(&memory, bounds);
        
        assert!(result.is_err());
        if let Err(SignatureExtractionError::MemoryAccess(msg)) = result {
            assert!(msg.contains("Memory trap occurred"));
        } else {
            panic!("Expected MemoryAccess error");
        }
    }
}