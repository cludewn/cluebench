// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Versioned authoring data. This contract intentionally does not invoke Core
//! validation: incomplete inputs remain editable after a save/open round trip.

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use std::{
    collections::BTreeSet,
    fmt,
    io::{self, Write},
};
use uuid::{Uuid, Variant, Version};
use zeroize::Zeroizing;

pub(crate) const MAX_JSON_BYTES: usize = 256 * 1024;
pub(crate) const MAX_DECODED_STRING_BYTES: usize = 96 * 1024;
pub(crate) const MAX_DEPTH: usize = 16;
pub(crate) const MAX_NODES: usize = 4096;
pub(crate) const MAX_OBJECTS: usize = 256;
pub(crate) const MAX_FIELD_BYTES: usize = 64;
pub(crate) const MAX_ENTITIES: usize = 72;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ProjectError {
    #[serde(rename = "PROJECT_INVALID_DOCUMENT")]
    InvalidDocument,
    #[serde(rename = "PROJECT_UNSUPPORTED_VERSION")]
    UnsupportedVersion,
    #[serde(rename = "PROJECT_BUDGET_EXCEEDED")]
    BudgetExceeded,
    #[serde(rename = "PROJECT_INVALID_PASSPHRASE")]
    InvalidPassphrase,
    #[serde(rename = "PROJECT_AUTHENTICATION_FAILED")]
    AuthenticationFailed,
    #[serde(rename = "PROJECT_CRYPTO_FAILED")]
    CryptoFailed,
}

impl ProjectError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidDocument => "PROJECT_INVALID_DOCUMENT",
            Self::UnsupportedVersion => "PROJECT_UNSUPPORTED_VERSION",
            Self::BudgetExceeded => "PROJECT_BUDGET_EXCEEDED",
            Self::InvalidPassphrase => "PROJECT_INVALID_PASSPHRASE",
            Self::AuthenticationFailed => "PROJECT_AUTHENTICATION_FAILED",
            Self::CryptoFailed => "PROJECT_CRYPTO_FAILED",
        }
    }
}

// Serde's derived record visitor also accepts positional sequences. Persisted
// records must be JSON objects, so expose only visit_map to the JSON parser.
macro_rules! record {
    ($name:ident { $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct $name { $($(#[$attr])* pub(crate) $field: $ty),* }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct RecordVisitor;
                impl<'de> Visitor<'de> for RecordVisitor {
                    type Value = $name;
                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a project record object")
                    }
                    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                        #[derive(Deserialize)]
                        #[serde(rename_all = "camelCase", deny_unknown_fields)]
                        struct Fields { $($(#[$attr])* $field: $ty),* }
                        let fields = Fields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                        Ok($name { $($field: fields.$field),* })
                    }
                }
                deserializer.deserialize_map(RecordVisitor)
            }
        }
    };
}

macro_rules! tagged_record {
    ($name:ident { $($variant:ident { $($(#[$attr:meta])* $field:ident: $ty:ty),* $(,)? }),* $(,)? }) => {
        #[derive(Clone, PartialEq, Eq, Serialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        pub(crate) enum $name { $($variant { $($(#[$attr])* $field: $ty),* }),* }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct RecordVisitor;
                impl<'de> Visitor<'de> for RecordVisitor {
                    type Value = $name;
                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a tagged project record object")
                    }
                    fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                        #[derive(Deserialize)]
                        #[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
                        enum Fields { $($variant { $($(#[$attr])* $field: $ty),* }),* }
                        let fields = Fields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                        Ok(match fields { $(Fields::$variant { $($field),* } => $name::$variant { $($field),* }),* })
                    }
                }
                deserializer.deserialize_map(RecordVisitor)
            }
        }
    };
}

record!(ProjectDocumentV1 {
    format: String,
    schema_version: u32,
    clues: Vec<ProjectClue>,
    hypotheses: Vec<ProjectHypothesis>,
    groups: Vec<ProjectGroup>,
    direct_constraints: Vec<ProjectDirectConstraint>,
    manual_seeds: ProjectManualSeeds,
    options: ProjectOptions,
    limits: ProjectLimits,
});
record!(ProjectClue {
    id: String,
    text: String,
    source: String
});
record!(ProjectHypothesis {
    id: String,
    origin: ProjectOrigin,
    interpretation: String,
    applicability: Applicability,
    confidence: Confidence,
    activation: ProjectActivation,
    effects: Vec<ProjectEffect>,
});
record!(ProjectEffect {
    id: String,
    constraint: ConstraintDraftV1
});
record!(ProjectGroup {
    id: String,
    anchor_clue_id: String,
    #[serde(deserialize_with = "required_nullable")]
    selected_hypothesis_id: Option<String>,
});
record!(ProjectDirectConstraint {
    id: String,
    enabled: bool,
    applicability: Applicability,
    confidence: Confidence,
    constraint: ConstraintDraftV1,
});
record!(ProjectManualSeeds { text: String });
record!(ProjectOptions {
    case_variants: Vec<ProjectCase>,
    prefixes: AffixDraftV1,
    suffixes: AffixDraftV1,
});
record!(AffixDraftV1 { include_empty: bool, rows: Vec<ProjectAffixRow> });
record!(ProjectAffixRow {
    id: String,
    value: String
});
record!(ProjectLimits {
    attempt_limit: String,
    emission_limit: String
});

fn required_nullable<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

tagged_record!(ProjectOrigin {
    Clue {
        #[serde(rename = "clueId")]
        clue_id: String,
    },
});
tagged_record!(ProjectActivation {
    Independent {
        enabled: bool,
    },
    Grouped {
        #[serde(rename = "groupId")]
        group_id: String,
    },
});
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Applicability {
    Hard,
    Soft,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Confidence {
    Low,
    Medium,
    High,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectCase {
    Original,
    AsciiLowercase,
    AsciiUppercase,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CharacterClass {
    Lowercase,
    Uppercase,
    Digit,
    Space,
    Symbol,
}

// Derived unit enums also accept externally tagged objects. The document
// contract requires string values, including inside buffered tagged records.
macro_rules! string_enum {
    ($name:ident { $($wire:literal => $variant:ident),* $(,)? }) => {
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct StringVisitor;
                impl Visitor<'_> for StringVisitor {
                    type Value = $name;
                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a project enum string")
                    }
                    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        match value {
                            $($wire => Ok($name::$variant),)*
                            _ => Err(E::custom("invalid project enum")),
                        }
                    }
                }
                deserializer.deserialize_str(StringVisitor)
                    .map_err(|_| de::Error::custom("invalid project enum"))
            }
        }
    };
}

string_enum!(Applicability { "hard" => Hard, "soft" => Soft });
string_enum!(Confidence { "low" => Low, "medium" => Medium, "high" => High });
string_enum!(ProjectCase {
    "original" => Original,
    "asciiLowercase" => AsciiLowercase,
    "asciiUppercase" => AsciiUppercase,
});
string_enum!(CharacterClass {
    "lowercase" => Lowercase,
    "uppercase" => Uppercase,
    "digit" => Digit,
    "space" => Space,
    "symbol" => Symbol,
});

tagged_record!(ConstraintDraftV1 {
    Length {
        min: String,
        max: String,
    },
    Position {
        index: String,
        allowed: String,
    },
    Contains {
        literal: String,
    },
    NotContains {
        literal: String,
    },
    Prefix {
        literal: String,
    },
    Suffix {
        literal: String,
    },
    CharacterCount {
        class: CharacterClass,
        min: String,
        max: String,
    },
});

impl ProjectDocumentV1 {
    pub(crate) fn validate(&self) -> Result<(), ProjectError> {
        if self.schema_version != 1 {
            return Err(ProjectError::UnsupportedVersion);
        }
        if self.format != "cluebench-project" {
            return Err(ProjectError::InvalidDocument);
        }
        if self.clues.len() > 8
            || self.hypotheses.len() > 8
            || self.groups.len() > 8
            || self.direct_constraints.len() > 16
            || self.options.prefixes.rows.len() > 8
            || self.options.suffixes.rows.len() > 8
            || self.hypotheses.iter().any(|h| h.effects.len() > 4)
            || self
                .hypotheses
                .iter()
                .map(|h| h.effects.len())
                .sum::<usize>()
                > 16
        {
            return Err(ProjectError::BudgetExceeded);
        }
        let mut ids = BTreeSet::new();
        let mut identity = |id: &String| -> Result<(), ProjectError> {
            let parsed = Uuid::parse_str(id).map_err(|_| ProjectError::InvalidDocument)?;
            if parsed.get_version() != Some(Version::Random)
                || parsed.get_variant() != Variant::RFC4122
                || parsed.hyphenated().to_string() != *id
                || !ids.insert(id.clone())
            {
                return Err(ProjectError::InvalidDocument);
            }
            Ok(())
        };
        for clue in &self.clues {
            identity(&clue.id)?;
            text(&clue.text, 512, TextDomain::Evidence)?;
            text(&clue.source, 256, TextDomain::Evidence)?;
        }
        for hypothesis in &self.hypotheses {
            identity(&hypothesis.id)?;
            text(&hypothesis.interpretation, 256, TextDomain::Evidence)?;
            let ProjectOrigin::Clue { clue_id } = &hypothesis.origin;
            if !self.clues.iter().any(|c| &c.id == clue_id) {
                return Err(ProjectError::InvalidDocument);
            }
            if let ProjectActivation::Grouped { group_id } = &hypothesis.activation
                && !self
                    .groups
                    .iter()
                    .any(|g| &g.id == group_id && &g.anchor_clue_id == clue_id)
            {
                return Err(ProjectError::InvalidDocument);
            }
            for effect in &hypothesis.effects {
                identity(&effect.id)?;
                effect.constraint.validate()?;
            }
        }
        for group in &self.groups {
            identity(&group.id)?;
            if !self.clues.iter().any(|c| c.id == group.anchor_clue_id) {
                return Err(ProjectError::InvalidDocument);
            }
            if let Some(selected) = &group.selected_hypothesis_id
                && !self.hypotheses.iter().any(|h| &h.id == selected && matches!(&h.activation, ProjectActivation::Grouped{group_id} if group_id == &group.id)) {
                return Err(ProjectError::InvalidDocument);
            }
        }
        for direct in &self.direct_constraints {
            identity(&direct.id)?;
            direct.constraint.validate()?;
        }
        for affixes in [&self.options.prefixes, &self.options.suffixes] {
            for row in &affixes.rows {
                identity(&row.id)?;
                text(&row.value, 256, TextDomain::Affix)?;
            }
        }
        if ids.len() > MAX_ENTITIES {
            return Err(ProjectError::BudgetExceeded);
        }
        if self.options.case_variants.len() > 3
            || self
                .options
                .case_variants
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != self.options.case_variants.len()
        {
            return Err(ProjectError::InvalidDocument);
        }
        text(&self.manual_seeds.text, 16 * 1024, TextDomain::Evidence)?;
        text(&self.limits.attempt_limit, 256, TextDomain::Constraint)?;
        text(&self.limits.emission_limit, 256, TextDomain::Constraint)?;
        Ok(())
    }
}
impl ConstraintDraftV1 {
    fn validate(&self) -> Result<(), ProjectError> {
        match self {
            Self::Length { min, max } | Self::CharacterCount { min, max, .. } => {
                text(min, 256, TextDomain::Constraint)?;
                text(max, 256, TextDomain::Constraint)
            }
            Self::Position { index, allowed } => {
                text(index, 256, TextDomain::Constraint)?;
                text(allowed, 95, TextDomain::Constraint)
            }
            Self::Contains { literal }
            | Self::NotContains { literal }
            | Self::Prefix { literal }
            | Self::Suffix { literal } => text(literal, 64, TextDomain::Constraint),
        }
    }
}
#[derive(Clone, Copy)]
enum TextDomain {
    Evidence,
    Constraint,
    Affix,
}
fn text(value: &str, max: usize, domain: TextDomain) -> Result<(), ProjectError> {
    if value.len() > max {
        return Err(ProjectError::BudgetExceeded);
    }
    let invalid = match domain {
        TextDomain::Evidence => value.contains('\r'),
        TextDomain::Affix => value.contains(['\r', '\n']),
        TextDomain::Constraint => value.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}'),
    };
    if invalid {
        Err(ProjectError::InvalidDocument)
    } else {
        Ok(())
    }
}

/// Counts decoded strings (including field names), values, and containers
/// without constructing a generic JSON tree or losing duplicate map keys.
#[derive(Default)]
struct Budget {
    nodes: usize,
    objects: usize,
    strings: usize,
    exceeded: bool,
}
impl Budget {
    fn charge<E: de::Error>(
        &mut self,
        depth: usize,
        strings: usize,
        object: bool,
    ) -> Result<(), E> {
        self.nodes += 1;
        self.objects += usize::from(object);
        self.strings += strings;
        if depth > MAX_DEPTH
            || self.nodes > MAX_NODES
            || self.objects > MAX_OBJECTS
            || self.strings > MAX_DECODED_STRING_BYTES
        {
            self.exceeded = true;
            return Err(E::custom("project budget"));
        }
        Ok(())
    }
}
struct Scan<'a> {
    budget: &'a mut Budget,
    depth: usize,
}
impl<'de> DeserializeSeed<'de> for Scan<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        self.budget.charge::<D::Error>(self.depth, 0, false)?;
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Scan<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        self.budget.strings += value.len();
        if self.budget.strings > MAX_DECODED_STRING_BYTES {
            self.budget.exceeded = true;
            Err(E::custom("project budget"))
        } else {
            Ok(())
        }
    }
    fn visit_seq<M: SeqAccess<'de>>(self, mut seq: M) -> Result<(), M::Error> {
        while seq
            .next_element_seed(Scan {
                budget: self.budget,
                depth: self.depth + 1,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        self.budget.objects += 1;
        if self.budget.objects > MAX_OBJECTS {
            self.budget.exceeded = true;
            return Err(de::Error::custom("project budget"));
        }
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > MAX_FIELD_BYTES {
                self.budget.exceeded = true;
                return Err(de::Error::custom("project budget"));
            }
            self.budget.strings += key.len();
            if self.budget.strings > MAX_DECODED_STRING_BYTES {
                self.budget.exceeded = true;
                return Err(de::Error::custom("project budget"));
            }
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate field"));
            }
            map.next_value_seed(Scan {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
        }
        Ok(())
    }
}
fn preflight(bytes: &[u8]) -> Result<(), ProjectError> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(ProjectError::BudgetExceeded);
    }
    let mut budget = Budget::default();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let parsed = Scan {
        budget: &mut budget,
        depth: 1,
    }
    .deserialize(&mut deserializer);
    if parsed.is_err() {
        return Err(if budget.exceeded {
            ProjectError::BudgetExceeded
        } else {
            ProjectError::InvalidDocument
        });
    }
    deserializer
        .end()
        .map_err(|_| ProjectError::InvalidDocument)
}
pub(crate) fn parse_document(bytes: &[u8]) -> Result<ProjectDocumentV1, ProjectError> {
    preflight(bytes)?;
    let document: ProjectDocumentV1 =
        serde_json::from_slice(bytes).map_err(|_| ProjectError::InvalidDocument)?;
    document.validate()?;
    Ok(document)
}
struct BoundedWriter {
    bytes: Zeroizing<Vec<u8>>,
}
impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > MAX_JSON_BYTES - self.bytes.len() {
            return Err(io::Error::other("project budget"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn serialize_document(
    document: &ProjectDocumentV1,
) -> Result<Zeroizing<Vec<u8>>, ProjectError> {
    document.validate()?;
    let mut writer = BoundedWriter {
        bytes: Zeroizing::new(Vec::new()),
    };
    serde_json::to_writer(&mut writer, document).map_err(|_| ProjectError::BudgetExceeded)?;
    preflight(&writer.bytes)?;
    Ok(writer.bytes)
}

#[cfg(test)]
pub(crate) fn synthetic_document() -> ProjectDocumentV1 {
    fn id(n: u64) -> String {
        format!("00000000-0000-4000-8000-{n:012x}")
    }
    ProjectDocumentV1 {
        format: "cluebench-project".into(),
        schema_version: 1,
        clues: vec![ProjectClue {
            id: id(1),
            text: " Synthetic 🧩\n\t".into(),
            source: "".into(),
        }],
        hypotheses: vec![
            ProjectHypothesis {
                id: id(2),
                origin: ProjectOrigin::Clue { clue_id: id(1) },
                interpretation: "".into(),
                applicability: Applicability::Soft,
                confidence: Confidence::Low,
                activation: ProjectActivation::Grouped { group_id: id(4) },
                effects: vec![ProjectEffect {
                    id: id(3),
                    constraint: ConstraintDraftV1::Length {
                        min: "0002".into(),
                        max: "-".into(),
                    },
                }],
            },
            ProjectHypothesis {
                id: id(5),
                origin: ProjectOrigin::Clue { clue_id: id(1) },
                interpretation: "".into(),
                applicability: Applicability::Hard,
                confidence: Confidence::High,
                activation: ProjectActivation::Independent { enabled: false },
                effects: vec![],
            },
        ],
        groups: vec![ProjectGroup {
            id: id(4),
            anchor_clue_id: id(1),
            selected_hypothesis_id: Some(id(2)),
        }],
        direct_constraints: vec![ProjectDirectConstraint {
            id: id(6),
            enabled: false,
            applicability: Applicability::Hard,
            confidence: Confidence::Medium,
            constraint: ConstraintDraftV1::Position {
                index: "invalid数字".into(),
                allowed: "".into(),
            },
        }],
        manual_seeds: ProjectManualSeeds {
            text: "\nSynthetic\tseed\n\n非ASCII\n".into(),
        },
        options: ProjectOptions {
            case_variants: vec![],
            prefixes: AffixDraftV1 {
                include_empty: true,
                rows: vec![
                    ProjectAffixRow {
                        id: id(7),
                        value: "".into(),
                    },
                    ProjectAffixRow {
                        id: id(8),
                        value: "重复".into(),
                    },
                    ProjectAffixRow {
                        id: id(9),
                        value: "重复".into(),
                    },
                ],
            },
            suffixes: AffixDraftV1 {
                include_empty: false,
                rows: vec![],
            },
        },
        limits: ProjectLimits {
            attempt_limit: "0002".into(),
            emission_limit: "".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(n: u64) -> String {
        format!("00000000-0000-4000-8000-{n:012x}")
    }
    fn invalid(document: &ProjectDocumentV1) {
        assert!(serialize_document(document).is_err());
    }
    fn bytes(document: &ProjectDocumentV1) -> Zeroizing<Vec<u8>> {
        serialize_document(document).unwrap()
    }

    fn assert_string_enum<T>(variants: &[(&str, T)])
    where
        T: serde::de::DeserializeOwned + Serialize + PartialEq,
    {
        for (name, expected) in variants {
            let encoded = format!("\"{name}\"");
            let parsed = serde_json::from_str::<T>(&encoded).unwrap();
            assert!(&parsed == expected);
            assert_eq!(serde_json::to_string(expected).unwrap(), encoded);
            assert!(serde_json::from_str::<T>(&format!("{{\"{name}\":null}}")).is_err());
        }
        for invalid in [
            "{}",
            "[]",
            "[\"soft\"]",
            "0",
            "1.5",
            "true",
            "false",
            "null",
            "\"unknown\"",
            "\"\"",
        ] {
            assert!(serde_json::from_str::<T>(invalid).is_err());
        }
    }

    #[test]
    fn applicability_requires_string_enum_values() {
        assert_string_enum(&[("hard", Applicability::Hard), ("soft", Applicability::Soft)]);
    }

    #[test]
    fn confidence_requires_string_enum_values() {
        assert_string_enum(&[
            ("low", Confidence::Low),
            ("medium", Confidence::Medium),
            ("high", Confidence::High),
        ]);
    }

    #[test]
    fn project_case_requires_string_enum_values() {
        assert_string_enum(&[
            ("original", ProjectCase::Original),
            ("asciiLowercase", ProjectCase::AsciiLowercase),
            ("asciiUppercase", ProjectCase::AsciiUppercase),
        ]);
    }

    #[test]
    fn character_class_requires_string_enum_values() {
        assert_string_enum(&[
            ("lowercase", CharacterClass::Lowercase),
            ("uppercase", CharacterClass::Uppercase),
            ("digit", CharacterClass::Digit),
            ("space", CharacterClass::Space),
            ("symbol", CharacterClass::Symbol),
        ]);
    }

    #[test]
    fn object_valued_enums_are_rejected_at_the_document_boundary() {
        let mut document = synthetic_document();
        document.options.case_variants = vec![ProjectCase::Original];
        document.direct_constraints[0].constraint = ConstraintDraftV1::CharacterCount {
            class: CharacterClass::Digit,
            min: "".into(),
            max: "-".into(),
        };
        let serialized = String::from_utf8(bytes(&document).to_vec()).unwrap();
        for (valid, invalid) in [
            (
                "\"applicability\":\"soft\"",
                "\"applicability\":{\"soft\":null}",
            ),
            ("\"confidence\":\"low\"", "\"confidence\":{\"low\":null}"),
            (
                "\"caseVariants\":[\"original\"]",
                "\"caseVariants\":[{\"original\":null}]",
            ),
            ("\"class\":\"digit\"", "\"class\":{\"digit\":null}"),
        ] {
            let changed = serialized.replacen(valid, invalid, 1);
            assert!(changed != serialized);
            assert_eq!(
                parse_document(changed.as_bytes())
                    .err()
                    .map(ProjectError::code),
                Some("PROJECT_INVALID_DOCUMENT")
            );
        }
    }

    #[test]
    fn incomplete_raw_authoring_round_trip_preserves_identity_order_and_all_states() {
        let mut document = synthetic_document();
        document.options.case_variants = vec![
            ProjectCase::AsciiUppercase,
            ProjectCase::Original,
            ProjectCase::AsciiLowercase,
        ];
        for (n, constraint) in [
            ConstraintDraftV1::Length {
                min: "64".into(),
                max: "1".into(),
            },
            ConstraintDraftV1::Contains { literal: "".into() },
            ConstraintDraftV1::NotContains {
                literal: "非ASCII".into(),
            },
            ConstraintDraftV1::Prefix {
                literal: " ".into(),
            },
            ConstraintDraftV1::Suffix { literal: "".into() },
            ConstraintDraftV1::CharacterCount {
                class: CharacterClass::Symbol,
                min: "not a number".into(),
                max: "".into(),
            },
        ]
        .into_iter()
        .enumerate()
        {
            document.direct_constraints.push(ProjectDirectConstraint {
                id: id(10 + n as u64),
                enabled: true,
                applicability: Applicability::Soft,
                confidence: Confidence::High,
                constraint,
            });
        }
        let first = bytes(&document);
        let parsed = parse_document(&first).unwrap();
        assert!(document == parsed);
        assert_eq!(first.as_slice(), bytes(&parsed).as_slice());
        let mut nonselected = document.clone();
        nonselected.groups[0].selected_hypothesis_id = None;
        assert!(parse_document(&bytes(&nonselected)).unwrap() == nonselected);
    }
    #[test]
    fn strict_records_require_every_field_and_explicit_nullable_selection() {
        let document = synthetic_document();
        let serialized = String::from_utf8(bytes(&document).to_vec()).unwrap();
        for malformed in [
            serialized.replacen("\"schemaVersion\":1,", "", 1),
            serialized.replacen("\"format\":\"cluebench-project\",", "", 1),
            serialized.replacen("\"text\":\" Synthetic 🧩\\n\\t\",", "", 1),
            serialized.replacen(&format!(",\"selectedHypothesisId\":\"{}\"", id(2)), "", 1),
            serialized.replacen("\"enabled\":false", "\"enabled\":null", 1),
            serialized.replacen("\"schemaVersion\":1", "\"schemaVersion\":\"1\"", 1),
            serialized.replacen("\"confidence\":\"low\"", "\"confidence\":\"certain\"", 1),
            serialized.replacen("\"includeEmpty\":true", "\"includeEmpty\":0", 1),
            serialized.replacen("\"options\":{", "\"options\":{\"extra\":true,", 1),
            serialized.replacen("\"origin\":{", "\"origin\":{\"extra\":true,", 1),
        ] {
            assert!(malformed != serialized);
            assert!(parse_document(malformed.as_bytes()).is_err());
        }
        for version in [0, 2, u32::MAX] {
            let changed = serialized.replacen(
                "\"schemaVersion\":1",
                &format!("\"schemaVersion\":{version}"),
                1,
            );
            assert!(matches!(
                parse_document(changed.as_bytes()),
                Err(ProjectError::UnsupportedVersion)
            ));
        }
    }
    #[test]
    fn duplicate_fields_are_rejected_before_enum_buffering_or_key_loss() {
        let serialized = String::from_utf8(bytes(&synthetic_document()).to_vec()).unwrap();
        for (needle, replacement) in [
            (
                "\"schemaVersion\":1",
                "\"schemaVersion\":1,\"schemaVersion\":1",
            ),
            ("\"text\":", "\"text\":\"duplicate\",\"text\":"),
            (
                "\"type\":\"length\"",
                "\"type\":\"length\",\"min\":\"duplicate\"",
            ),
            ("\"type\":\"clue\"", "\"type\":\"clue\",\"type\":\"clue\""),
            (
                "\"type\":\"grouped\"",
                "\"type\":\"grouped\",\"groupId\":\"duplicate\"",
            ),
        ] {
            let changed = serialized.replacen(needle, replacement, 1);
            assert!(parse_document(changed.as_bytes()).is_err());
        }
        // Keys are compared after JSON unescaping.
        let changed = serialized.replacen(
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"schema\\u0056ersion\":1",
            1,
        );
        assert!(parse_document(changed.as_bytes()).is_err());
    }
    #[test]
    fn positional_records_malformed_json_and_invalid_unicode_are_rejected() {
        let serialized = String::from_utf8(bytes(&synthetic_document()).to_vec()).unwrap();
        for malformed in [
            "[]".to_owned(),
            "null".into(),
            "{".into(),
            format!("{serialized}{{}}"),
            serialized
                .replacen("\"manualSeeds\":{\"text\":", "\"manualSeeds\":[", 1)
                .replacen("非ASCII\\n\"}", "非ASCII\\n\"]", 1),
            serialized.replacen(
                "\"limits\":{\"attemptLimit\":\"0002\",\"emissionLimit\":\"\"}",
                "\"limits\":[\"0002\",\"\"]",
                1,
            ),
            serialized.replacen("\"source\":\"\"", "\"source\":\"\\ud800\"", 1),
        ] {
            assert!(parse_document(malformed.as_bytes()).is_err());
        }
        assert!(parse_document(&[0xff, 0xfe]).is_err());
        for (object, positional) in [
            (
                format!("{{\"type\":\"clue\",\"clueId\":\"{}\"}}", id(1)),
                format!("[\"clue\",\"{}\"]", id(1)),
            ),
            (
                format!("{{\"type\":\"grouped\",\"groupId\":\"{}\"}}", id(4)),
                format!("[\"grouped\",\"{}\"]", id(4)),
            ),
            (
                "{\"type\":\"length\",\"min\":\"0002\",\"max\":\"-\"}".into(),
                "[\"length\",\"0002\",\"-\"]".into(),
            ),
        ] {
            let changed = serialized.replacen(&object, &positional, 1);
            assert!(changed != serialized);
            assert!(parse_document(changed.as_bytes()).is_err());
        }
    }
    #[test]
    fn uuid_uniqueness_version_variant_and_kind_references_are_validated() {
        let original = synthetic_document();
        let mut document = original.clone();
        document.clues[0].id = id(6);
        invalid(&document);
        for bad in [
            "not-uuid",
            "00000000-0000-1000-8000-000000000001",
            "00000000-0000-4000-c000-000000000001",
            "00000000-0000-4000-8000-00000000000A",
            "00000000000040008000000000000001",
        ] {
            let mut document = original.clone();
            document.options.prefixes.rows[0].id = bad.into();
            invalid(&document);
        }
        let mut document = original.clone();
        document.hypotheses[0].origin = ProjectOrigin::Clue { clue_id: id(6) };
        invalid(&document);
        let mut document = original.clone();
        document.hypotheses[0].activation = ProjectActivation::Grouped { group_id: id(6) };
        invalid(&document);
        let mut document = original.clone();
        document.groups[0].anchor_clue_id = id(777);
        invalid(&document);
        let mut document = original.clone();
        document.groups[0].selected_hypothesis_id = Some(id(5));
        invalid(&document);
        let mut document = original.clone();
        document.groups[0].selected_hypothesis_id = Some(id(777));
        invalid(&document);
        let mut document = original;
        document.clues.push(ProjectClue {
            id: id(888),
            text: "".into(),
            source: "".into(),
        });
        document.groups[0].anchor_clue_id = id(888);
        invalid(&document);
    }
    #[test]
    fn text_domains_match_editors_without_normalization_and_use_utf8_bytes() {
        let mut document = synthetic_document();
        document.clues[0].text = "é".repeat(256);
        document.clues[0].source = "x".repeat(256);
        document.hypotheses[0].interpretation = "x".repeat(256);
        document.manual_seeds.text = "\n\t".repeat(8192);
        document.options.prefixes.rows[0].value = "é".repeat(128);
        document.limits.attempt_limit = "-".repeat(256);
        document.direct_constraints[0].constraint = ConstraintDraftV1::Position {
            index: "x".repeat(256),
            allowed: "x".repeat(95),
        };
        assert!(parse_document(&bytes(&document)).unwrap() == document);
        let mut over = document.clone();
        over.clues[0].text.push('x');
        invalid(&over);
        let mut over = document.clone();
        over.clues[0].source.push('x');
        invalid(&over);
        let mut over = document.clone();
        over.hypotheses[0].interpretation.push('x');
        invalid(&over);
        let mut over = document.clone();
        over.manual_seeds.text.push('x');
        invalid(&over);
        let mut over = document.clone();
        over.options.prefixes.rows[0].value.push('x');
        invalid(&over);
        let mut over = document.clone();
        over.limits.attempt_limit.push('x');
        invalid(&over);
        for cr in ["\r", "\r\n"] {
            let mut bad = document.clone();
            bad.manual_seeds.text = cr.into();
            invalid(&bad);
            let mut bad = document.clone();
            bad.clues[0].text = cr.into();
            invalid(&bad);
        }
        for line in ["\r", "\n"] {
            let mut bad = document.clone();
            bad.options.prefixes.rows[0].value = line.into();
            invalid(&bad);
        }
        for control in ["\0", "\t", "\n", "\r", "\u{7f}"] {
            let mut bad = document.clone();
            bad.limits.attempt_limit = control.into();
            invalid(&bad);
            bad.limits.attempt_limit = "1".into();
            bad.direct_constraints[0].constraint = ConstraintDraftV1::Contains {
                literal: control.into(),
            };
            invalid(&bad);
        }
        let mut literal = document.clone();
        literal.direct_constraints[0].constraint = ConstraintDraftV1::Contains {
            literal: "é".repeat(32),
        };
        assert!(literal.validate().is_ok());
        literal.direct_constraints[0].constraint = ConstraintDraftV1::Contains {
            literal: "é".repeat(33),
        };
        invalid(&literal);
    }
    #[test]
    fn per_kind_entity_limits_and_eight_rows_with_include_empty_are_independent() {
        let mut document = synthetic_document();
        document.clues = (0..8)
            .map(|n| ProjectClue {
                id: id(100 + n),
                text: "x".repeat(512),
                source: "x".repeat(256),
            })
            .collect();
        document.hypotheses = (0..8)
            .map(|n| ProjectHypothesis {
                id: id(200 + n),
                origin: ProjectOrigin::Clue { clue_id: id(100) },
                interpretation: "x".repeat(256),
                applicability: Applicability::Hard,
                confidence: Confidence::High,
                activation: ProjectActivation::Grouped { group_id: id(300) },
                effects: (0..2)
                    .map(|e| ProjectEffect {
                        id: id(400 + n * 2 + e),
                        constraint: ConstraintDraftV1::Length {
                            min: "".into(),
                            max: "-".into(),
                        },
                    })
                    .collect(),
            })
            .collect();
        document.groups = (0..8)
            .map(|n| ProjectGroup {
                id: id(300 + n),
                anchor_clue_id: id(100),
                selected_hypothesis_id: None,
            })
            .collect();
        document.direct_constraints = (0..16)
            .map(|n| ProjectDirectConstraint {
                id: id(500 + n),
                enabled: true,
                applicability: Applicability::Hard,
                confidence: Confidence::Low,
                constraint: ConstraintDraftV1::Prefix { literal: "".into() },
            })
            .collect();
        document.options.prefixes.rows = (0..8)
            .map(|n| ProjectAffixRow {
                id: id(600 + n),
                value: "".into(),
            })
            .collect();
        document.options.suffixes.rows = (0..8)
            .map(|n| ProjectAffixRow {
                id: id(700 + n),
                value: "".into(),
            })
            .collect();
        assert!(bytes(&document).len() > 16 * 1024);
        assert!(parse_document(&bytes(&document)).unwrap() == document);
        let mut bad = document.clone();
        bad.clues.push(ProjectClue {
            id: id(999),
            text: "".into(),
            source: "".into(),
        });
        invalid(&bad);
        let mut bad = document.clone();
        bad.hypotheses.push(document.hypotheses[0].clone());
        invalid(&bad);
        let mut bad = document.clone();
        bad.groups.push(document.groups[0].clone());
        invalid(&bad);
        let mut bad = document.clone();
        bad.direct_constraints
            .push(document.direct_constraints[0].clone());
        invalid(&bad);
        let mut bad = document.clone();
        bad.options.prefixes.rows.push(ProjectAffixRow {
            id: id(999),
            value: "".into(),
        });
        invalid(&bad);
        let mut bad = document.clone();
        bad.hypotheses[0].effects.push(ProjectEffect {
            id: id(999),
            constraint: ConstraintDraftV1::Suffix { literal: "".into() },
        });
        invalid(&bad);
        let mut bad = document;
        bad.hypotheses[0].effects = (0..5)
            .map(|n| ProjectEffect {
                id: id(900 + n),
                constraint: ConstraintDraftV1::Suffix { literal: "".into() },
            })
            .collect();
        invalid(&bad);
    }
    #[test]
    fn independent_parser_budgets_stop_at_the_document_boundary() {
        assert!(preflight(&vec![b' '; MAX_JSON_BYTES + 1]).is_err());
        let mut exact = vec![b' '; MAX_JSON_BYTES];
        exact[0] = b'0';
        assert!(preflight(&exact).is_ok());
        let exact_string = format!("\"{}\"", "x".repeat(MAX_DECODED_STRING_BYTES));
        assert!(preflight(exact_string.as_bytes()).is_ok());
        let over_string = format!("\"{}\"", "x".repeat(MAX_DECODED_STRING_BYTES + 1));
        assert_eq!(
            preflight(over_string.as_bytes()),
            Err(ProjectError::BudgetExceeded)
        );
        let field = format!("{{\"{}\":0}}", "x".repeat(64));
        assert!(preflight(field.as_bytes()).is_ok());
        let field = format!("{{\"{}\":0}}", "x".repeat(65));
        assert_eq!(
            preflight(field.as_bytes()),
            Err(ProjectError::BudgetExceeded)
        );
        let nested = format!("{}0{}", "[".repeat(15), "]".repeat(15));
        assert!(preflight(nested.as_bytes()).is_ok());
        let nested = format!("{}0{}", "[".repeat(16), "]".repeat(16));
        assert_eq!(
            preflight(nested.as_bytes()),
            Err(ProjectError::BudgetExceeded)
        );
        let nodes = format!("[{}]", vec!["0"; MAX_NODES - 1].join(","));
        assert!(preflight(nodes.as_bytes()).is_ok());
        let nodes = format!("[{}]", vec!["0"; MAX_NODES].join(","));
        assert_eq!(
            preflight(nodes.as_bytes()),
            Err(ProjectError::BudgetExceeded)
        );
        let objects = format!("[{}]", vec!["{}"; MAX_OBJECTS].join(","));
        assert!(preflight(objects.as_bytes()).is_ok());
        let objects = format!("[{}]", vec!["{}"; MAX_OBJECTS + 1].join(","));
        assert_eq!(
            preflight(objects.as_bytes()),
            Err(ProjectError::BudgetExceeded)
        );
    }
}
