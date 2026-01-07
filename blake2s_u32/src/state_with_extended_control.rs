use core::mem::MaybeUninit;

// NOTE: here we need struct definition for external crates, but we will panic in implementations instead

use crate::aligned_array::AlignedArray64;

use super::*;

// Here we try different approach to Blake round function, but placing extra burden
// into "precompile" in terms of control flow

#[cfg(all(target_arch = "riscv32", feature = "blake2_with_compression"))]
use common_constants::delegation_types::blake2s_with_control::*;

// we will pass
// - mutable ptr to state + extended state (basically - to self),
// with words 12 and 14 set in the extended state to what we need if we do not use "compression" mode
// - const ptr to input (that may be treated differently)
// - round mask
// - control register: output_flag || is_right flag for compression || compression mode flag

// WORKAROUND: Use fence instructions on RV64 to prevent compiler optimization bugs.
// The CSR instruction reads/writes memory, so we need to ensure memory is synced before/after.
// RV32 doesn't have these issues and Airbender's simulator doesn't implement fence.
#[cfg(all(target_arch = "riscv64", feature = "blake2_with_compression"))]
#[inline(never)]
fn csr_trigger_delegation(
    states_ptr: *mut u32,
    input_ptr: *const u32,
    round_mask: u32,
    control_mask: u32,
) {
    use core::hint::black_box;
    use core::sync::atomic::{compiler_fence, Ordering};

    let states_ptr = black_box(states_ptr);
    let input_ptr = black_box(input_ptr);

    compiler_fence(Ordering::SeqCst);

    // Force memory to be visible
    unsafe {
        let _ = core::ptr::read_volatile(states_ptr as *const u8);
        let _ = core::ptr::read_volatile(input_ptr as *const u8);
    }

    unsafe {
        core::arch::asm!(
            "fence rw, rw",
            "csrrw x0, 0x7c7, x0",
            "fence rw, rw",
            in("x10") states_ptr.addr(),
            in("x11") input_ptr.addr(),
            in("x12") round_mask,
            in("x13") control_mask,
            options(nostack, preserves_flags)
        )
    }

    // Force memory sync after CSR
    unsafe {
        let val = core::ptr::read_volatile(states_ptr as *const u8);
        core::ptr::write_volatile(states_ptr as *mut u8, val);
    }

    compiler_fence(Ordering::SeqCst);
}

#[cfg(all(target_arch = "riscv32", feature = "blake2_with_compression"))]
#[inline(always)]
fn csr_trigger_delegation(
    states_ptr: *mut u32,
    input_ptr: *const u32,
    round_mask: u32,
    control_mask: u32,
) {
    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7c7, x0",
            in("x10") states_ptr.addr(),
            in("x11") input_ptr.addr(),
            in("x12") round_mask,
            in("x13") control_mask,
            options(nostack, preserves_flags)
        )
    }
}

#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
const NORMAL_MODE_FIRST_ROUNDS_CONTROL_REGISTER: u32 = 0b000;
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
const NORMAL_MODE_LAST_ROUND_CONTROL_REGISTER: u32 = 0b001;
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
const COMPRESSION_MODE_FIRST_ROUNDS_BASE_CONTROL_REGISTER: u32 = 0b100;
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
const COMPRESSION_MODE_LAST_ROUND_EXTRA_BITS: u32 = 0b001;
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
const COMPRESSION_MODE_IS_RIGHT_EXTRA_BITS: u32 = 0b010;

#[derive(Clone, Copy, Debug)]
#[repr(C, align(128))]
pub struct Blake2RoundFunctionEvaluator {
    pub state: [u32; BLAKE2S_STATE_WIDTH_IN_U32_WORDS], // represents current state
    extended_state: [u32; BLAKE2S_EXTENDED_STATE_WIDTH_IN_U32_WORDS], // represents scratch space for evaluation
    // there is no input buffer, and we will use registers to actually pass control flow flags
    // there will be special buffer for witness to write into, that
    // we will take care to initialize, even though we will use only half of it
    pub input_buffer: AlignedArray64<u32, BLAKE2S_BLOCK_SIZE_U32_WORDS>,
    t: u32, // we limit ourselves to <4Gb inputs
}

impl Blake2RoundFunctionEvaluator {
    pub const SUPPORT_SPEC_SINGLE_ROUND: bool = false;

    #[unroll::unroll_for_loops]
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            state: BLAKE2S_IV,
            extended_state: [0u32; BLAKE2S_EXTENDED_STATE_WIDTH_IN_U32_WORDS],
            input_buffer: AlignedArray64::default(),
            t: 0,
        }
    }

    #[unroll::unroll_for_loops]
    #[inline(always)]
    pub fn reset(&mut self) {
        self.state = BLAKE2S_IV;
        for i in 0..BLAKE2S_EXTENDED_STATE_WIDTH_IN_U32_WORDS {
            self.extended_state[i] = 0;
        }
        self.t = 0;
    }

    #[unroll::unroll_for_loops]
    pub fn absorb_multiple_blocks(&mut self, input: &[u8]) {
        let block_len = BLAKE2S_BLOCK_SIZE_IN_BYTES;
        if input.len() % block_len != 0 {
            panic!()
        }
        for chunk in input.chunks_exact(block_len) {
            self.absorb_block(chunk)
        }
    }

    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    #[unroll::unroll_for_loops]
    pub fn absorb_block(&mut self, input: &[u8]) {
        for (dst, src) in self.input_buffer.0.iter_mut().zip(input.array_chunks::<4>()) {
            *dst = u32::from_le_bytes(*src);
        }

        self.absorb_prepared_block()
    }

    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    #[inline(always)]
    pub fn absorb_block(&mut self, _input: &[u8]) {
        unimplemented!()
    }

    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    #[unroll::unroll_for_loops]
    #[inline(always)]
    pub fn absorb_prepared_block(&mut self) {
        self.t += BLAKE2S_BLOCK_SIZE_IN_BYTES as u32;

        // init extended state
        self.extended_state[0..8].copy_from_slice(&BLAKE2S_IV);
        self.extended_state[12] = self.t;
        self.extended_state[14] = 0;

        let state_ptr: *mut u32 = self.state.as_mut_ptr();
        let input_ptr: *const u32 = self.input_buffer.0.as_ptr();

        // all rounds except the last
        for round in 0..9 {
            let round_mask = ROUND_FUNCTION_MASK[round];
            csr_trigger_delegation(
                state_ptr,
                input_ptr,
                round_mask,
                NORMAL_MODE_FIRST_ROUNDS_CONTROL_REGISTER,
            );
        }

        // then handle final round
        let round_mask = ROUND_FUNCTION_MASK[9];
        csr_trigger_delegation(
            state_ptr,
            input_ptr,
            round_mask,
            NORMAL_MODE_LAST_ROUND_CONTROL_REGISTER,
        );
    }

    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    #[inline(always)]
    pub fn absorb_prepared_block(&mut self) {
        unimplemented!()
    }

    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    #[unroll::unroll_for_loops]
    pub fn finalize_without_digest_length(
        mut self,
        remaining_input: &[u8],
        dst: &mut MaybeUninit<[u8; 32]>,
    ) {
        debug_assert!(remaining_input.len() < BLAKE2S_BLOCK_SIZE_IN_BYTES);

        self.t += remaining_input.len() as u32;
        // init padding with zeroes
        for el in self.input_buffer.0.iter_mut() {
            *el = 0u32;
        }

        // note the endianess of course
        for (idx, src) in remaining_input.iter().enumerate() {
            let word = idx / 4;
            let byte_offset = (idx % 4) * 8;
            self.input_buffer.0[word] |= (*src as u32) << byte_offset;
        }

        // init extended state
        self.extended_state[0..8].copy_from_slice(&BLAKE2S_IV);
        self.extended_state[12] = self.t;
        self.extended_state[14] = 0xffff_ffff;

        let state_ptr: *mut u32 = self.state.as_mut_ptr();
        let input_ptr: *const u32 = self.input_buffer.0.as_ptr();

        // all rounds except the last
        for round in 0..9 {
            let round_mask = ROUND_FUNCTION_MASK[round];
            csr_trigger_delegation(
                state_ptr,
                input_ptr,
                round_mask,
                NORMAL_MODE_FIRST_ROUNDS_CONTROL_REGISTER,
            );
        }

        // then handle final round
        let round_mask = ROUND_FUNCTION_MASK[9];
        csr_trigger_delegation(
            state_ptr,
            input_ptr,
            round_mask,
            NORMAL_MODE_LAST_ROUND_CONTROL_REGISTER,
        );

        // write into dst
        let dst = dst.as_mut_ptr().cast::<[u32; 8]>();
        unsafe { dst.write(self.state) };
    }

    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    #[inline(always)]
    pub fn finalize_without_digest_length(
        self,
        _remaining_input: &[u8],
        _dst: &mut MaybeUninit<[u8; 32]>,
    ) {
        unimplemented!()
    }

    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    #[unroll::unroll_for_loops]
    pub fn finalize_with_digest_length(
        mut self,
        digest_length: usize,
        remaining_input: &[u8],
        dst: &mut MaybeUninit<[u8; 32]>,
    ) {
        debug_assert!(remaining_input.len() < BLAKE2S_BLOCK_SIZE_IN_BYTES);
        debug_assert!(digest_length <= 32);

        // apply personalization
        let param = [digest_length as u32 | (1 << 16) | (1 << 24), 0, 0, 0, 0, 0, 0, 0];
        for i in 0..8 {
            self.state[i] ^= param[i];
        }

        self.t += remaining_input.len() as u32;
        // init padding with zeroes
        for el in self.input_buffer.0.iter_mut() {
            *el = 0u32;
        }

        // note the endianess of course
        for (idx, src) in remaining_input.iter().enumerate() {
            let word = idx / 4;
            let byte_offset = (idx % 4) * 8;
            self.input_buffer.0[word] |= (*src as u32) << byte_offset;
        }

        // init extended state
        self.extended_state[0..8].copy_from_slice(&BLAKE2S_IV);
        self.extended_state[12] = self.t;
        self.extended_state[14] = 0xffff_ffff;

        let state_ptr: *mut u32 = self.state.as_mut_ptr();
        let input_ptr: *const u32 = self.input_buffer.0.as_ptr();

        // all rounds except the last
        for round in 0..9 {
            let round_mask = ROUND_FUNCTION_MASK[round];
            csr_trigger_delegation(
                state_ptr,
                input_ptr,
                round_mask,
                NORMAL_MODE_FIRST_ROUNDS_CONTROL_REGISTER,
            );
        }

        // then handle final round
        let round_mask = ROUND_FUNCTION_MASK[9];
        csr_trigger_delegation(
            state_ptr,
            input_ptr,
            round_mask,
            NORMAL_MODE_LAST_ROUND_CONTROL_REGISTER,
        );

        // write into dst
        let dst = dst.as_mut_ptr().cast::<[u32; 8]>();
        unsafe { dst.write(self.state) };
    }

    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    #[inline(always)]
    pub fn finalize_with_digest_length(
        self,
        _digest_length: usize,
        _remaining_input: &[u8],
        _dst: &mut MaybeUninit<[u8; 32]>,
    ) {
        unimplemented!()
    }

    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    #[inline(always)]
    pub fn two_to_one_compression_fixed_length(
        mut self,
        left: &[u8; 32],
        right: &[u8; 32],
        dst: &mut MaybeUninit<[u8; 32]>,
    ) {
        // we need to follow the usual HAIFA structure
        // - h0 = IV XOR params
        // - h1 = F(h0, 0..0 || left, 32)
        // - h2 = F(h1, 0..0 || right, 64 | FINAL_FLAG)

        // param is digest_len (32 bytes) || key_len (0) || fanout (1) || depth (1)
        let param = [32 | (1 << 16) | (1 << 24), 0, 0, 0, 0, 0, 0, 0];
        for i in 0..8 {
            self.state[i] ^= param[i];
        }

        // in compression mode we place data into input in specific order and locations
        // so first we compress left, then right
        // input buffer is separate from state + extended state, and we write left and right
        // consecutively in it

        // first we do left
        for (dst, src) in self.input_buffer.0[..8].iter_mut().zip(left.array_chunks::<4>()) {
            *dst = u32::from_le_bytes(*src);
        }

        let t: u32 = 32; // we absorbed 32 bytes
                         // init extended state
        self.extended_state[0..8].copy_from_slice(&BLAKE2S_IV);
        self.extended_state[12] = t;
        self.extended_state[14] = 0;

        let state_ptr: *mut u32 = self.state.as_mut_ptr();
        let input_ptr: *const u32 = self.input_buffer.0.as_ptr();

        // all rounds except the last
        for round in 0..9 {
            let round_mask = ROUND_FUNCTION_MASK[round];
            let control_mask = COMPRESSION_MODE_FIRST_ROUNDS_BASE_CONTROL_REGISTER;
            csr_trigger_delegation(state_ptr, input_ptr, round_mask, control_mask);
        }

        // then handle final round
        let round_mask = ROUND_FUNCTION_MASK[9];
        let control_mask =
            COMPRESSION_MODE_FIRST_ROUNDS_BASE_CONTROL_REGISTER | COMPRESSION_MODE_LAST_ROUND_EXTRA_BITS;
        csr_trigger_delegation(state_ptr, input_ptr, round_mask, control_mask);

        // now we do right

        for (dst, src) in self.input_buffer.0[8..16].iter_mut().zip(right.array_chunks::<4>()) {
            *dst = u32::from_le_bytes(*src);
        }

        let t: u32 = 64; // we absorbed 64 bytes
                         // init extended state
        self.extended_state[0..8].copy_from_slice(&BLAKE2S_IV);
        self.extended_state[12] = t;
        self.extended_state[14] = 0xffff_ffff; // mark as final

        // all rounds except the last
        for round in 0..9 {
            let round_mask = ROUND_FUNCTION_MASK[round];
            let control_mask =
                COMPRESSION_MODE_FIRST_ROUNDS_BASE_CONTROL_REGISTER | COMPRESSION_MODE_IS_RIGHT_EXTRA_BITS;
            csr_trigger_delegation(state_ptr, input_ptr, round_mask, control_mask);
        }

        // then handle final round
        let round_mask = ROUND_FUNCTION_MASK[9];
        let control_mask = COMPRESSION_MODE_FIRST_ROUNDS_BASE_CONTROL_REGISTER
            | COMPRESSION_MODE_IS_RIGHT_EXTRA_BITS
            | COMPRESSION_MODE_LAST_ROUND_EXTRA_BITS;
        csr_trigger_delegation(state_ptr, input_ptr, round_mask, control_mask);

        // write into dst
        let dst = dst.as_mut_ptr().cast::<[u32; 8]>();
        unsafe { dst.write(self.state) };
    }

    #[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
    #[inline(always)]
    pub fn two_to_one_compression_fixed_length(
        self,
        _left: &[u8; 32],
        _right: &[u8; 32],
        _dst: &mut MaybeUninit<[u8; 32]>,
    ) {
        unimplemented!()
    }
}
