// Adapted from https://github.com/succinctlabs/sp1/tree/dev/crates/zkvm/entrypoint/src
// This is musl-libc commit 37e18b7bf307fa4a8c745feebfcba54a0ba74f30:
//
// src/string/memcpy.c
//
// This was compiled into assembly with:
//
// clang -target riscv64 -march=rv64imac -mabi=lp64 -O3 -S memcpy.c -nostdlib -fno-builtin -funroll-loops
//
// and labels manually updated to not conflict.
//
// musl as a whole is licensed under the following standard MIT license:
//
// ----------------------------------------------------------------------
// Copyright © 2005-2020 Rich Felker, et al.
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
// SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
// ----------------------------------------------------------------------
	.text
	.attribute	4, 16
	.attribute	5, "rv64im"
	.file	"musl_memcpy64.c"
	.globl	memcpy
	.p2align	2
	.type	memcpy,@function
memcpy:
	xor	a3, a1, a0
	andi	a3, a3, 7
	beqz	a3, .LBBmemcpy64_6
	mv	a4, a0
.LBBmemcpy64_2:
	mv	a5, a1
.LBBmemcpy64_3:
	beqz	a2, .LBBmemcpy64_16
	add	a2, a2, a4
.LBBmemcpy64_5:
	lbu	a1, 0(a5)
	addi	a5, a5, 1
	addi	a3, a4, 1
	sb	a1, 0(a4)
	mv	a4, a3
	bne	a3, a2, .LBBmemcpy64_5
	j	.LBBmemcpy64_16
.LBBmemcpy64_6:
	andi	a3, a0, 7
	snez	a7, a2
	beqz	a3, .LBBmemcpy64_17
	beqz	a2, .LBBmemcpy64_15
	addi	a5, a0, 1
	li	a6, 1
	mv	a3, a0
.LBBmemcpy64_9:
	mv	a4, a2
	lbu	a7, 0(a1)
	addi	a1, a1, 1
	addi	a2, a2, -1
	andi	t0, a5, 7
	sb	a7, 0(a3)
	addi	a3, a3, 1
	snez	a7, a2
	beqz	t0, .LBBmemcpy64_11
	addi	a5, a5, 1
	bne	a4, a6, .LBBmemcpy64_9
.LBBmemcpy64_11:
	beqz	a7, .LBBmemcpy64_16
.LBBmemcpy64_12:
	li	a4, 8
	bltu	a2, a4, .LBBmemcpy64_18
	li	a6, 7
.LBBmemcpy64_14:
	ld	a7, 0(a1)
	addi	a5, a1, 8
	addi	a4, a3, 8
	addi	a2, a2, -8
	sd	a7, 0(a3)
	mv	a1, a5
	mv	a3, a4
	bltu	a6, a2, .LBBmemcpy64_14
	j	.LBBmemcpy64_3
.LBBmemcpy64_15:
	mv	a3, a0
	bnez	a7, .LBBmemcpy64_12
.LBBmemcpy64_16:
	ret
.LBBmemcpy64_17:
	mv	a3, a0
	bnez	a7, .LBBmemcpy64_12
	j	.LBBmemcpy64_16
.LBBmemcpy64_18:
	mv	a4, a3
	j	.LBBmemcpy64_2
.Lfuncmemcpy64_end0:
	.size	memcpy, .Lfuncmemcpy64_end0-memcpy

	.section	".note.GNU-stack","",@progbits
	.addrsig
