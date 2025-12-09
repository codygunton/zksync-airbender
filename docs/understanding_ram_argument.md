# Understanding the RAM Argument

This guide provides a structured reading path to understand how the permutation-based memory argument works in Airbender.

## 📚 Documentation (Start Here!)
1. **`docs/subarguments_used.md`** - Theory and explanation of the permutation-based memory argument

## 🏗️ Core Definitions & Data Structures
2. **`cs/src/definitions/ram_access.rs`** - Core RAM access data structures (`ShuffleRamQueryColumns`, address types)
3. **`cs/src/definitions/memory_tree.rs`** - Memory tree structure for Merkle commitments
4. **`cs/src/definitions/stage2.rs`** - Stage 2 layout including memory argument intermediates
5. **`cs/src/definitions/witness_tree.rs`** - Witness tree organization
6. **`cs/src/definitions/setup_tree.rs`** - Setup tree for timestamps

## 🔧 Machine Operations (Constraints)
7. **`cs/src/machine/ops/load.rs`** - Load instruction constraints (how reads work)
8. **`cs/src/machine/ops/store.rs`** - Store instruction constraints (how writes work)
9. **`cs/src/machine/ops/mod.rs`** - Common RAM operation patterns

## 📊 Tracing (Collecting Memory Accesses)
10. **`prover/src/tracers/main_cycle_optimized.rs`** - Main cycle tracer that collects RAM accesses
11. **`prover/src/tracers/delegation.rs`** - Delegation circuit tracing

## 🧮 Witness Generation (Prover Side)
12. **`prover/src/witness_evaluator/memory_witness/main_circuit.rs`** - Memory witness for main RISC-V circuit
13. **`prover/src/witness_evaluator/memory_witness/delegation_circuit.rs`** - Memory witness for delegation circuits
14. **`prover/src/prover_stages/stage1.rs`** - Pre-commitment stage for memory

## ✅ Verification (Verifier Side)
15. **`verifier_generator/src/inlining_generator/memory_accumulators.rs`** - Verifier code generation for memory argument checking
16. **`verifier/src/lib.rs`** - See lines 484-486 for memory argument challenges usage

## 🖥️ GPU Implementation (Optional - for performance)
17. **`gpu_prover/src/witness/memory_main.rs`** - GPU-accelerated memory witness for main circuit
18. **`gpu_prover/src/witness/memory_delegation.rs`** - GPU-accelerated memory witness for delegation
19. **`gpu_prover/src/witness/ram_access.rs`** - GPU RAM access helpers

## 📐 Constraint Compilation
20. **`cs/src/one_row_compiler/compile_layout.rs`** - How RAM constraints are compiled into the circuit
21. **`cs/src/cs/circuit.rs`** - Circuit structure including RAM argument integration

## 🎯 Example Usage
22. **`circuit_defs/trace_and_split/src/lib.rs`** - See lines 62-74 for memory initialization example
23. **`prover/src/witness_evaluator/ext_calls_with_gpu_tracers.rs`** - Lines 78-84 show memory setup

## 📖 Suggested Reading Order

1. Start with **`docs/subarguments_used.md`** (theory)
2. Read **`cs/src/definitions/ram_access.rs`** (data structures)
3. Look at **`cs/src/machine/ops/load.rs`** and **`store.rs`** (how constraints work)
4. Check **`prover/src/tracers/main_cycle_optimized.rs`** (how accesses are collected)
5. Examine **`prover/src/witness_evaluator/memory_witness/main_circuit.rs`** (witness generation)
6. Finally **`verifier_generator/src/inlining_generator/memory_accumulators.rs`** (verification)

## Theory Overview

The RAM argument is based on the paper ["Two Shuffles Make a RAM"](https://eprint.iacr.org/2023/1115.pdf). It proves memory consistency by showing:

**`init + write_set` is a permutation of `teardown + read_set`**

Each memory access creates tuples:
- **Read**: `(address, read_timestamp, read_value)`
- **Write**: `(address, write_timestamp, write_value)`
- Constraint: `read_ts < write_ts`

### Key Benefits

1. **Free range checks**: If an address/value appears in the permutation, it's automatically range-checked
2. **Timestamps enforce ordering**: Can only read from "the past"
3. **Registers as RAM**: Registers are modeled as RAM with address space `(is_register_bool, u32_index)`
4. **38-bit timestamps**: Pair of 19-bit integers allows up to 2³⁶ cycles
