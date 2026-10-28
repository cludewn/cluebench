// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Fixed inclusive limits on logical input sizes, not allocator capacities.

pub const MAX_ENTITIES: usize = 4096;
pub const MAX_HYPOTHESES: usize = 1024;
pub const MAX_EFFECTS: usize = 2048;
pub const MAX_REFERENCE_EDGES: usize = 4096;
pub const MAX_STRING_BYTES: usize = 4096;
pub const MAX_TOTAL_STRING_BYTES: usize = 1_048_576;
pub const MAX_DIAGNOSTICS: usize = 64;
pub const MAX_CANDIDATE_BYTES: usize = 64;
pub const MAX_SOFT_POINTS: u32 = 3072;

pub const MAX_CASE_OPTIONS: usize = 3;
pub const MAX_PREFIX_OPTIONS: usize = 128;
pub const MAX_SUFFIX_OPTIONS: usize = 128;
pub const MAX_AFFIX_BYTES: usize = 64;
/// Logical configuration payload: cases (one byte each), affix bytes, and two
/// u64 allowances (16 bytes). Container metadata is bounded by option counts.
pub const MAX_GENERATION_CONFIG_BYTES: usize = 8192;
pub const MAX_GENERATION_ATTEMPTS: u64 = 1_000_000;
pub const MAX_GENERATION_EMISSIONS: u64 = 1_000_000;
