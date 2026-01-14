/*
    Entry point of all programs (_start) for RV64.

    It initializes DWARF call frame information, the stack pointer, the
    frame pointer (needed for closures to work in start_rust) and the global
    pointer. Then it calls _start_rust.
*/

.section .init, "ax"
.global _start

_start:
    la ra, _abs_start
    jr ra

_abs_start:
    .cfi_startproc
    .cfi_undefined ra

    .option push
    .option norelax
    la gp, __global_pointer$
    .option pop

    // Assume single core, and put SP to the very top address of the stack region
    la sp, _sstack

    // Set frame pointer
    add s0, sp, zero

    jal zero, _start_rust

    .cfi_endproc

/*
    Machine trap entry point (_machine_start_trap) for RV64
*/
.section .trap, "ax"
.global machine_default_start_trap
.align 4
machine_default_start_trap:
    // We assume that exception stack is always saved to MSCRATCH

    // so we swap it with x31
	csrrw x31, mscratch, x31

    // write to exception stack using sd (store double) for RV64
    sd x30, -16(x31)
    sd x29, -24(x31)
    sd x28, -32(x31)
    sd x27, -40(x31)
    sd x26, -48(x31)
    sd x25, -56(x31)
    sd x24, -64(x31)
    sd x23, -72(x31)
    sd x22, -80(x31)
    sd x21, -88(x31)
    sd x20, -96(x31)
    sd x19, -104(x31)
    sd x18, -112(x31)
    sd x17, -120(x31)
    sd x16, -128(x31)
    sd x15, -136(x31)
    sd x14, -144(x31)
    sd x13, -152(x31)
    sd x12, -160(x31)
    sd x11, -168(x31)
    sd x10, -176(x31)
    sd x9, -184(x31)
    sd x8, -192(x31)
    sd x7, -200(x31)
    sd x6, -208(x31)
    sd x5, -216(x31)
    sd x4, -224(x31)
    sd x3, -232(x31)
    sd x2, -240(x31)
    sd x1, -248(x31)

    // move valid sp into a0,
    mv a0, x31
    csrrw x31, mscratch, x0
    sd x31, -8(a0)
    // restore sp
    mv sp, a0
    // sp is valid now

    addi sp, sp, -256
    // pass pointer as first argument
    add a0, sp, zero

    jal ra, _machine_start_trap_rust

    // set return address into mepc
    csrw mepc, a0

    // save original SP to mscratch for now
    ld a0, 16(sp) // it's original sp that we saved in the stack
    csrw mscratch, a0 // save it for now

    // restore everything we saved

    ld x1, 8(sp)
    // ld x2, 16(sp) // do not overwrite SP yet
    ld x3, 24(sp)
    ld x4, 32(sp)
    ld x5, 40(sp)
    ld x6, 48(sp)
    ld x7, 56(sp)
    ld x8, 64(sp)
    ld x9, 72(sp)
    ld x10, 80(sp)
    ld x11, 88(sp)
    ld x12, 96(sp)
    ld x13, 104(sp)
    ld x14, 112(sp)
    ld x15, 120(sp)
    ld x16, 128(sp)
    ld x17, 136(sp)
    ld x18, 144(sp)
    ld x19, 152(sp)
    ld x20, 160(sp)
    ld x21, 168(sp)
    ld x22, 176(sp)
    ld x23, 184(sp)
    ld x24, 192(sp)
    ld x25, 200(sp)
    ld x26, 208(sp)
    ld x27, 216(sp)
    ld x28, 224(sp)
    ld x29, 232(sp)
    ld x30, 240(sp)
    ld x31, 248(sp)

    addi sp, sp, 256
    // we popped everything from the stack
    // now save current exception SP to mscratch,
    // and put original SP back
    csrrw	sp, mscratch, sp

    mret

/* Make sure there is an abort when linking */
.section .text.abort
.global abort
abort:
    j abort
