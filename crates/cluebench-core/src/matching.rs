// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Shared matching semantics for validated constraints and supported candidates.
//! Every helper entry and examined byte pair/character reserves one work unit
//! before doing that work. Callers choose whether to enforce a budget.

use crate::{CharacterClass, Constraint};

pub(crate) fn matches_constraint<E>(
    constraint: &Constraint,
    candidate: &str,
    work: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<bool, E> {
    work(1)?;
    match constraint {
        Constraint::Length { min, max } => Ok((*min..=*max).contains(&candidate.len())),
        Constraint::Position { index, allowed } => {
            let Some(required) = candidate.as_bytes().get(index - 1) else {
                return Ok(false);
            };
            for alternative in allowed.as_bytes() {
                work(1)?;
                if alternative == required {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Constraint::Contains(literal) => contains(candidate.as_bytes(), literal.as_bytes(), work),
        Constraint::NotContains(literal) => {
            Ok(!contains(candidate.as_bytes(), literal.as_bytes(), work)?)
        }
        Constraint::Prefix(literal) => starts_with(candidate.as_bytes(), literal.as_bytes(), work),
        Constraint::Suffix(literal) => {
            let bytes = candidate.as_bytes();
            if literal.len() > bytes.len() {
                return Ok(false);
            }
            starts_with(
                &bytes[bytes.len() - literal.len()..],
                literal.as_bytes(),
                work,
            )
        }
        Constraint::CharacterCount { class, min, max } => {
            let mut count = 0;
            for byte in candidate.as_bytes() {
                work(1)?;
                if character_class(*byte) == *class {
                    count += 1;
                }
            }
            Ok((*min..=*max).contains(&count))
        }
    }
}

/// Naive bounded substring matching: no allocation, hidden searcher setup, or
/// uncharged bulk comparison. Inputs originate in the validated ASCII domain.
pub(crate) fn contains<E>(
    haystack: &[u8],
    needle: &[u8],
    work: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<bool, E> {
    work(1)?;
    if needle.len() > haystack.len() {
        return Ok(false);
    }
    if needle.is_empty() {
        return Ok(true);
    }
    for offset in 0..=haystack.len() - needle.len() {
        if starts_with(&haystack[offset..], needle, work)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn starts_with<E>(
    value: &[u8],
    prefix: &[u8],
    work: &mut impl FnMut(u64) -> Result<(), E>,
) -> Result<bool, E> {
    work(1)?;
    if prefix.len() > value.len() {
        return Ok(false);
    }
    for index in 0..prefix.len() {
        work(1)?;
        if value[index] != prefix[index] {
            return Ok(false);
        }
    }
    Ok(true)
}

fn character_class(byte: u8) -> CharacterClass {
    match byte {
        b'a'..=b'z' => CharacterClass::Lowercase,
        b'A'..=b'Z' => CharacterClass::Uppercase,
        b'0'..=b'9' => CharacterClass::Digit,
        b' ' => CharacterClass::Space,
        _ => CharacterClass::Symbol,
    }
}
