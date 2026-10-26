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
