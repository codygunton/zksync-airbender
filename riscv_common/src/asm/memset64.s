// Adapted from https://github.com/succinctlabs/sp1/tree/dev/crates/zkvm/entrypoint/src
// This is musl-libc memset commit 37e18b7bf307fa4a8c745feebfcba54a0ba74f30:
//
// src/string/memset.c
//
// This was compiled into assembly with:
//
// clang -target riscv64 -march=rv64imac -mabi=lp64 -O3 -S memset.c -nostdlib -fno-builtin -funroll-loops
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
	.file	"musl_memset64.c"
	.globl	memset
	.p2align	2
	.type	memset,@function
memset:
	beqz	a2, .LBBmemset64_14
	sb	a1, 0(a0)
	add	a3, a0, a2
	li	a4, 3
	sb	a1, -1(a3)
	bltu	a2, a4, .LBBmemset64_14
	sb	a1, 1(a0)
	sb	a1, 2(a0)
	li	a4, 7
	sb	a1, -3(a3)
	sb	a1, -2(a3)
	bltu	a2, a4, .LBBmemset64_14
	sb	a1, 3(a0)
	li	a4, 9
	sb	a1, -4(a3)
	bltu	a2, a4, .LBBmemset64_14
	negw	a3, a0
	andi	a4, a1, 255
	lui	a1, 4112
	andi	a3, a3, 3
	addi	a5, a1, 257
	add	a1, a0, a3
	sub	a2, a2, a3
	slli	a3, a5, 32
	andi	a2, a2, -4
	add	a3, a3, a5
	andi	a5, a1, 4
	mul	a3, a4, a3
	beqz	a5, .LBBmemset64_6
	sw	a3, 0(a1)
	addi	a1, a1, 4
	addi	a2, a2, -4
.LBBmemset64_6:
	li	a4, 32
	bltu	a2, a4, .LBBmemset64_9
	li	a4, 31
.LBBmemset64_8:
	sd	a3, 0(a1)
	sd	a3, 8(a1)
	sd	a3, 16(a1)
	sd	a3, 24(a1)
	addi	a2, a2, -32
	addi	a1, a1, 32
	bltu	a4, a2, .LBBmemset64_8
.LBBmemset64_9:
	li	a4, 8
	bltu	a2, a4, .LBBmemset64_12
	li	a4, 7
.LBBmemset64_11:
	sd	a3, 0(a1)
	addi	a2, a2, -8
	addi	a1, a1, 8
	bltu	a4, a2, .LBBmemset64_11
.LBBmemset64_12:
	li	a4, 4
	bltu	a2, a4, .LBBmemset64_14
	sw	a3, 0(a1)
.LBBmemset64_14:
	ret
.Lfuncmemset64_end0:
	.size	memset, .Lfuncmemset64_end0-memset

	.section	".note.GNU-stack","",@progbits
	.addrsig
