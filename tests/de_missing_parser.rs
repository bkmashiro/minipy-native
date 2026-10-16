//! DE: dependency-level proof only. No source-language get or runtime changes.
#![cfg(feature = "tool-json")]

use serde::de::IgnoredAny;
use std::ops::Range;

// Same spatial invariant as JsonState::locate, exercised on arbitrary subranges.
fn selected_range(owner: &[u8], within: Range<usize>, path: &[&str]) -> Range<usize> {
    let selected = sonic_rs::get(&owner[within.clone()], path.iter().copied()).unwrap();
    let raw = selected.as_raw_str().as_bytes();
    let start = (raw.as_ptr() as usize)
        .checked_sub(owner.as_ptr() as usize)
        .unwrap();
    let end = start.checked_add(raw.len()).unwrap();
    assert!(start >= within.start && end <= within.end);
    assert_eq!(&owner[start..end], raw);
    start..end
}

// Validate only an already-selected receiver. These stages are deliberately
// separate: IgnoredAny accepts non-objects and does not validate skipped UTF-8.
fn validate_object(bytes: &[u8]) -> Result<(), &'static str> {
    let text = std::str::from_utf8(bytes).map_err(|_| "utf8")?;
    let first = bytes
        .iter()
        .copied()
        .find(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'));
    if first != Some(b'{') {
        return Err("object");
    }
    serde_json::from_str::<IgnoredAny>(text).map_err(|_| "syntax")?;
    Ok(())
}

#[test]
fn final_key_classification_and_first_match() {
    for bytes in [b"{}".as_slice(), br#"{"other":1}"#] {
        let error = sonic_rs::get(bytes, ["owner"]).unwrap_err();
        assert!(error.is_not_found(), "{bytes:?}: {error}");
        assert_eq!(validate_object(bytes), Ok(()));
    }
    for (bytes, expected) in [
        (br#"{"owner":"yes"}"#.as_slice(), r#""yes""#),
        (br#"{"owner":""}"#, r#""""#),
        (br#"{"owner":"first","owner":"last"}"#, r#""first""#),
        (
            br#"{"ow\u006eer":"escaped","owner":"last"}"#,
            r#""escaped""#,
        ),
        (br#"{"owner":null,"owner":"last"}"#, "null"),
        (br#"{"owner":1}"#, "1"),
        (br#"{"owner":false}"#, "false"),
        (br#"{"owner":{}}"#, "{}"),
        (br#"{"owner":[]}"#, "[]"),
    ] {
        let found = sonic_rs::get(bytes, ["owner"]).unwrap();
        assert_eq!(found.as_raw_str(), expected);
        assert_eq!(validate_object(bytes), Ok(()));
    }
    for bytes in [b"null".as_slice(), b"1", b"false", br#""text""#, b"[]"] {
        let error = sonic_rs::get(bytes, ["owner"]).unwrap_err();
        assert!(!error.is_not_found(), "{bytes:?}: {error}");
        assert_eq!(validate_object(bytes), Err("object"));
    }
    // A multi-level NotFound does NOT identify which key is absent. Required
    // parent selection must finish first, without catching its error as default.
    assert!(
        sonic_rs::get(b"{}".as_slice(), ["meta", "owner"])
            .unwrap_err()
            .is_not_found()
    );
    assert!(
        sonic_rs::get(b"{}".as_slice(), ["meta"])
            .unwrap_err()
            .is_not_found()
    );
    let parent = sonic_rs::get(br#"{"meta":{}}"#.as_slice(), ["meta"]).unwrap();
    assert!(
        sonic_rs::get(parent.as_raw_str(), ["owner"])
            .unwrap_err()
            .is_not_found()
    );
    for bytes in [
        br#"{"meta":null}"#.as_slice(),
        br#"{"meta":[]}"#,
        br#"{"meta":"text"}"#,
    ] {
        let parent = sonic_rs::get(bytes, ["meta"]).unwrap();
        assert!(
            !sonic_rs::get(parent.as_raw_str(), ["owner"])
                .unwrap_err()
                .is_not_found()
        );
    }
    // NotFound also covers arrays when using an index; it is not an object proof.
    assert!(
        sonic_rs::get(b"[]".as_slice(), [0usize])
            .unwrap_err()
            .is_not_found()
    );
    assert!(
        sonic_rs::get(b"[1]".as_slice(), [1usize])
            .unwrap_err()
            .is_not_found()
    );
}

#[test]
fn false_missing_candidates_require_validation() {
    // Both bypass the successful-get UTF-8 check / strict first-key syntax.
    for (bytes, rejected_by) in [
        (b"{garbage}".as_slice(), "syntax"),
        (b"{\"other\":\"\xff\"}", "utf8"),
    ] {
        let error = sonic_rs::get(bytes, ["owner"]).unwrap_err();
        assert!(error.is_not_found(), "{bytes:?}: {error}");
        assert_eq!(validate_object(bytes), Err(rejected_by));
        eprintln!(
            "false-missing {bytes:?}: sonic={:?}, validator={rejected_by}",
            error.classify()
        );
    }
    // IgnoredAny alone cannot be used as the UTF-8 validator.
    assert!(serde_json::from_slice::<IgnoredAny>(b"{\"other\":\"\xff\"}").is_ok());
}

#[test]
fn selected_receiver_excludes_outer_suffix() {
    for suffix in [b"garbage".as_slice(), b",", b"\xff", b" {\"bad\":tru"] {
        let mut owner = b" \t\r\n{\"other\":1} ".to_vec();
        owner.extend_from_slice(suffix);
        let range = selected_range(&owner, 0..owner.len(), &[]);
        assert_eq!(range, 4..15);
        assert_eq!(&owner[range.clone()], br#"{"other":1}"#);
        assert_eq!(validate_object(&owner[range]), Ok(()));
        assert!(validate_object(&owner).is_err(), "whole input must fail");
    }
    // Missing outer terminator is deliberately not promoted into validation of
    // the root when the existing required path has selected a complete parent.
    let owner = br#"{"meta":{"other":1},"bad":tru"#;
    let parent = selected_range(owner, 0..owner.len(), &["meta"]);
    let receiver = selected_range(owner, parent.clone(), &[]);
    assert_eq!(receiver, parent);
    assert_eq!(&owner[receiver.clone()], br#"{"other":1}"#);
    assert_eq!(validate_object(&owner[receiver.clone()]), Ok(()));
    assert!(
        sonic_rs::get(&owner[receiver], ["owner"])
            .unwrap_err()
            .is_not_found()
    );
    assert!(validate_object(owner).is_err());
}

#[test]
fn malformed_selected_objects_never_validate() {
    for bytes in [
        b"{garbage}".as_slice(),
        br#"{"other":1 "next":2}"#,
        br#"{"other":1,}"#,
        br#"{"other" 1}"#,
        br#"{"other":tru}"#,
        br#"{"other":1"#,
        br#"{"other":[1,]}"#,
        br#"{"other":"\q"}"#,
        br#"{"other":"\uZZZZ"}"#,
        br#"{"other":"\u123"}"#,
        b"{\"other\":\"line\nbreak\"}",
    ] {
        assert_eq!(validate_object(bytes), Err("syntax"), "{bytes:?}");
        // safe selection may itself reject, but if it succeeds its exact raw
        // range must still fail the independent validator.
        match sonic_rs::get(bytes, std::iter::empty::<&str>()) {
            Ok(value) => assert_eq!(
                validate_object(value.as_raw_str().as_bytes()),
                Err("syntax")
            ),
            Err(error) => assert!(!error.is_not_found(), "{bytes:?}: {error}"),
        }
    }
    for bytes in [br#"{"other":1 "next":2}"#.as_slice(), br#"{"other":1"#] {
        assert!(!sonic_rs::get(bytes, ["owner"]).unwrap_err().is_not_found());
    }
    for bytes in [b"{\"other\":\"\xff\"}".as_slice(), b"{\"\xff\":1}"] {
        assert_eq!(validate_object(bytes), Err("utf8"));
        assert!(sonic_rs::get(bytes, std::iter::empty::<&str>()).is_err());
    }
}

#[test]
fn surrogate_syntax_is_not_string_domain_validation() {
    for (escaped, decoded) in [
        (r#""\uD800\uDC00""#, Some("𐀀")),
        (r#""\uDBFF\uDFFF""#, Some("\u{10ffff}")),
        (r#""\uD800""#, None),
        (r#""\uDC00""#, None),
        (r#""\uDC00\uD800""#, None),
        (r#""\uD800x""#, None),
    ] {
        let owner = format!(r#"{{"other":{escaped}}}"#);
        let range = selected_range(owner.as_bytes(), 0..owner.len(), &[]);
        assert_eq!(validate_object(&owner.as_bytes()[range]), Ok(()));
        assert!(
            sonic_rs::get(owner.as_bytes(), ["owner"])
                .unwrap_err()
                .is_not_found()
        );
        let string = sonic_rs::from_str::<String>(escaped);
        match decoded {
            Some(expected) => assert_eq!(string.unwrap(), expected),
            None => assert!(string.is_err(), "selected string must reject {escaped}"),
        }
    }
    // IgnoredAny also skips object keys without decoding their surrogate pairs.
    assert_eq!(validate_object(br#"{"\uD800":0}"#), Ok(()));
}

#[test]
fn existing_present_and_required_lazy_path_is_unchanged() {
    // A successful final lookup can ignore even an invalid sibling or missing
    // receiver terminator. The proposed missing-only proof must not run here.
    for bytes in [
        br#"{"owner":"yes","bad":tru}"#.as_slice(),
        br#"{"owner":"yes""#,
        b"{\"owner\":\"yes\",\"bad\":\"\xff\"}",
    ] {
        let found = sonic_rs::get(bytes, ["owner"]).unwrap();
        assert_eq!(
            sonic_rs::from_str::<String>(found.as_raw_str()).unwrap(),
            "yes"
        );
        assert!(validate_object(bytes).is_err());
    }
    // Existing [] reports missing, without any conversion to a default. A
    // selected containing object may already fail on its internal bad sibling.
    assert!(sonic_rs::get(b"{}".as_slice(), ["owner"]).is_err());
    assert!(
        sonic_rs::get(
            br#"{"meta":{"owner":"yes","bad":tru}}"#.as_slice(),
            ["meta"]
        )
        .is_err()
    );
    let outside_bad = br#"{"meta":{"count":10}"#;
    let parent = sonic_rs::get(outside_bad.as_slice(), ["meta"]).unwrap();
    let count = sonic_rs::get(parent.as_raw_str(), ["count"]).unwrap();
    assert_eq!(sonic_rs::from_str::<i64>(count.as_raw_str()).unwrap(), 10);
}
