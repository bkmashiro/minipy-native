//! Borrowed JSON access. No self-references, DOM, unchecked sonic APIs or user pointers.
use crate::{error::TaskError, tool::Plan};
use bytes::Bytes;
use std::ops::Range;

pub(crate) struct State {
    plan: std::rc::Rc<Plan>,
    roots: Vec<Option<Bytes>>,
    ranges: Vec<Option<Range<usize>>>,
    integers: Vec<Option<i64>>,
    strings: Option<Vec<StringCache>>,
    max: usize,
    external: usize,
    scan_left: usize,
}
enum StringCache {
    Unchecked,
    Validated,
    Decoded(String),
    Shared(std::sync::Arc<String>),
}
impl State {
    pub(crate) fn plan(&self) -> std::rc::Rc<Plan> {
        self.plan.clone()
    }
    pub(crate) fn new(plan: std::rc::Rc<Plan>, max: usize, scan_bytes: usize) -> Self {
        Self {
            roots: vec![None; plan.roots],
            ranges: vec![None; plan.paths.len()],
            integers: vec![None; plan.paths.len()],
            strings: plan.string_paths.then(|| {
                (0..plan.paths.len())
                    .map(|_| StringCache::Unchecked)
                    .collect()
            }),
            max,
            external: 0,
            scan_left: scan_bytes,
            plan,
        }
    }
    pub(crate) fn set_external(&mut self, bytes: usize) {
        self.external = bytes;
    }
    pub(crate) fn held(&self) -> usize {
        self.roots.iter().flatten().map(Bytes::len).sum::<usize>()
            + self.strings.as_ref().map_or(0, |d| {
                d.iter()
                    .map(|s| match s {
                        StringCache::Decoded(value) => value.capacity(),
                        StringCache::Shared(value) => value.capacity(),
                        _ => 0,
                    })
                    .sum()
            })
    }
    pub(crate) fn available(&self) -> usize {
        self.max
            .saturating_sub(self.external.saturating_add(self.held()))
    }
    pub(crate) fn install(
        &mut self,
        root: usize,
        bytes: Bytes,
        max: usize,
    ) -> Result<(), TaskError> {
        let old = self.roots[root].as_ref().map_or(0, Bytes::len);
        let old_decoded: usize = if old == 0 {
            0
        } else {
            self.plan
                .paths
                .iter()
                .enumerate()
                .filter(|(_, path)| path.root == root)
                .map(|(i, _)| match self.strings.as_ref().map(|s| &s[i]) {
                    Some(StringCache::Decoded(value)) => value.capacity(),
                    Some(StringCache::Shared(value)) => value.capacity(),
                    _ => 0,
                })
                .sum()
        };
        if bytes.len() > max.saturating_sub(self.held() - old - old_decoded) {
            return Err(TaskError::Budget);
        }
        if self.roots[root].is_some() {
            self.clear_root(root);
        }
        self.roots[root] = Some(bytes);
        Ok(())
    }
    fn clear_root(&mut self, root: usize) {
        for (i, path) in self.plan.paths.iter().enumerate() {
            if path.root == root {
                self.ranges[i] = None;
                self.integers[i] = None;
                if let Some(strings) = &mut self.strings {
                    strings[i] = StringCache::Unchecked;
                }
            }
        }
    }
    pub(crate) fn release(&mut self, live: u32) {
        for (root, bytes) in self.roots.iter_mut().enumerate() {
            if live & (1 << root) == 0 {
                *bytes = None;
            }
        }
        for (i, p) in self.plan.paths.iter().enumerate() {
            if live & (1 << p.root) == 0 {
                self.ranges[i] = None;
                self.integers[i] = None;
                if let Some(strings) = &mut self.strings {
                    strings[i] = StringCache::Unchecked;
                }
            }
        }
    }
    pub(crate) fn view(&mut self, slot: usize) -> Result<i64, TaskError> {
        self.locate(slot).map(|_| 0)
    }
    pub(crate) fn validate_string(&mut self, slot: usize) -> Result<i64, TaskError> {
        let len = self.locate(slot)?.len();
        self.charge(len)?;
        self.string(slot).map(|_| 0)
    }
    fn locate(&mut self, slot: usize) -> Result<Range<usize>, TaskError> {
        if let Some(r) = &self.ranges[slot] {
            return Ok(r.clone());
        }
        let p = &self.plan.paths[slot];
        let (root, parent) = (p.root, p.parent);
        let within = if let Some(parent) = parent {
            self.locate(parent)?
        } else {
            0..self.roots[root]
                .as_ref()
                .ok_or(TaskError::Json("released owner"))?
                .len()
        };
        let owner = self.roots[root]
            .as_ref()
            .ok_or(TaskError::Json("released owner"))?;
        let raw = sonic_rs::get(&owner[within.clone()], [self.plan.paths[slot].key.as_str()])
            .map_err(|_| TaskError::Json("missing field or invalid JSON on accessed path"))?;
        let range = Self::import_range(owner, within, raw.as_raw_str().as_bytes())?;
        self.ranges[slot] = Some(range.clone());
        Ok(range)
    }
    fn import_range(
        owner: &[u8],
        within: Range<usize>,
        raw: &[u8],
    ) -> Result<Range<usize>, TaskError> {
        // Borrowed sonic output must lie within our immutable input. Checked once
        // when importing the range, never turned into an arbitrary address.
        let offset = (raw.as_ptr() as usize)
            .checked_sub(owner.as_ptr() as usize)
            .ok_or(TaskError::Json("non-borrowed parser result"))?;
        let end = offset
            .checked_add(raw.len())
            .filter(|end| *end <= within.end)
            .ok_or(TaskError::Json("invalid parser range"))?;
        if offset < within.start {
            return Err(TaskError::Json("invalid parser range"));
        }
        Ok(offset..end)
    }
    /// Resolve required ancestors before treating one final key as optional.
    /// Found uses existing range/string caches; absence is never cached.
    pub(crate) fn get_present(&mut self, slot: usize) -> Result<bool, TaskError> {
        if self.ranges[slot].is_some() {
            return Ok(true);
        }
        let (root, parent) = {
            let p = &self.plan.paths[slot];
            (p.root, p.parent)
        };
        let within = if let Some(parent) = parent {
            self.locate(parent)?
        } else {
            0..self.roots[root]
                .as_ref()
                .ok_or(TaskError::Json("released owner"))?
                .len()
        };
        let owner = self.roots[root]
            .as_ref()
            .ok_or(TaskError::Json("released owner"))?;
        let receiver = &owner[within.clone()];
        // String-key NotFound must refer to an object, never an array/index.
        if receiver
            .iter()
            .copied()
            .find(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            != Some(b'{')
        {
            return Err(TaskError::Json("get receiver must be an object"));
        }
        match sonic_rs::get(receiver, [self.plan.paths[slot].key.as_str()]) {
            Ok(raw) => {
                let range = Self::import_range(owner, within, raw.as_raw_str().as_bytes())?;
                self.ranges[slot] = Some(range);
                Ok(true)
            }
            Err(e) if e.is_not_found() => {
                // DE: NotFound alone accepts malformed/invalid-UTF8 objects.
                // Select only the root object, excluding any external suffix.
                // A required parent's range is already exact.
                let range = if parent.is_none() {
                    let raw = sonic_rs::get(receiver, std::iter::empty::<&str>())
                        .map_err(|_| TaskError::Json("invalid missing-key receiver"))?;
                    Self::import_range(owner, within, raw.as_raw_str().as_bytes())?
                } else {
                    within
                };
                let text = std::str::from_utf8(&owner[range])
                    .map_err(|_| TaskError::Json("invalid missing-key receiver UTF-8"))?;
                serde_json::from_str::<serde::de::IgnoredAny>(text)
                    .map_err(|_| TaskError::Json("invalid missing-key receiver"))?;
                Ok(false)
            }
            Err(_) => Err(TaskError::Json(
                "missing field or invalid JSON on accessed path",
            )),
        }
    }
    pub(crate) fn integer(&mut self, slot: usize) -> Result<i64, TaskError> {
        if let Some(v) = self.integers[slot] {
            return Ok(v);
        }
        let range = self.locate(slot)?;
        let root = self.plan.paths[slot].root;
        let owner = self.roots[root]
            .as_ref()
            .ok_or(TaskError::Json("released owner"))?;
        let value = sonic_rs::from_slice::<i64>(&owner[range])
            .map_err(|_| TaskError::Json("expected signed-i64 integer"))?;
        self.integers[slot] = Some(value);
        Ok(value)
    }
    /// Plain strings borrow the immutable root; escaped strings decode once.
    pub(crate) fn string(&mut self, slot: usize) -> Result<&str, TaskError> {
        let range = self.locate(slot)?;
        let root = self.plan.paths[slot].root;
        if matches!(
            self.strings.as_ref().expect("string plan")[slot],
            StringCache::Unchecked
        ) {
            let raw = &self.roots[root]
                .as_ref()
                .ok_or(TaskError::Json("released owner"))?[range.clone()];
            if raw.len() > 64 * 1024 {
                return Err(TaskError::Budget);
            }
            if raw.first() != Some(&b'"') || raw.last() != Some(&b'"') || raw.len() < 2 {
                return Err(TaskError::Json("expected UTF-8 string"));
            }
            let state = if raw.contains(&b'\\') {
                self.charge(raw.len())?;
                let raw = &self.roots[root].as_ref().unwrap()[range.clone()];
                let decoded = sonic_rs::from_slice::<String>(raw)
                    .map_err(|_| TaskError::Json("expected UTF-8 string"))?;
                if decoded.capacity()
                    > self
                        .max
                        .saturating_sub(self.external.saturating_add(self.held()))
                {
                    return Err(TaskError::Budget);
                }
                StringCache::Decoded(decoded)
            } else {
                // Deserialization validates UTF-8 and JSON controls without a copy.
                sonic_rs::from_slice::<&str>(raw)
                    .map_err(|_| TaskError::Json("expected UTF-8 string"))?;
                StringCache::Validated
            };
            self.strings.as_mut().unwrap()[slot] = state;
        }
        Ok(self.prepared_string(slot))
    }
    /// Reborrow after string/validate_string succeeded, with no intervening
    /// install or release. The synchronous sink prepares every part first.
    pub(crate) fn prepared_string(&self, slot: usize) -> &str {
        match &self.strings.as_ref().expect("string plan")[slot] {
            StringCache::Decoded(value) => value,
            StringCache::Shared(value) => value,
            StringCache::Validated => {
                let root = self.plan.paths[slot].root;
                let range = self.ranges[slot].as_ref().expect("validated path").clone();
                let owner = self.roots[root].as_ref().expect("validated owner");
                // install/release invalidate the marker before changing the owner.
                unsafe { validated_span(owner, range) }
            }
            StringCache::Unchecked => unreachable!(),
        }
    }
    pub(crate) fn string_value(
        &mut self,
        slot: usize,
    ) -> Result<crate::tool::StringValue, TaskError> {
        self.validate_string(slot)?;
        let range = self.ranges[slot].as_ref().expect("validated path").clone();
        let cache = &mut self.strings.as_mut().expect("string plan")[slot];
        if matches!(cache, StringCache::Decoded(_)) {
            let StringCache::Decoded(s) = std::mem::replace(cache, StringCache::Unchecked) else {
                unreachable!()
            };
            *cache = StringCache::Shared(std::sync::Arc::new(s));
        }
        match cache {
            StringCache::Shared(s) => Ok(crate::tool::StringValue::Decoded(s.clone())),
            StringCache::Validated => {
                let root = self.plan.paths[slot].root;
                let owner = self.roots[root].as_ref().expect("validated owner").clone();
                Ok(crate::tool::StringValue::Json {
                    owner,
                    range: range.start + 1..range.end - 1,
                })
            }
            StringCache::Unchecked => unreachable!(),
            StringCache::Decoded(_) => unreachable!(),
        }
    }
    /// Bound waiting payload retention for a direct plain JSON Tool argument.
    /// Logical root length is only an eligibility heuristic, not backing capacity.
    pub(crate) fn string_argument(
        &mut self,
        slot: usize,
        live: u32,
    ) -> Result<crate::tool::StringValue, TaskError> {
        let value = self.string_value(slot)?;
        let root = self.plan.paths[slot].root;
        if live & (1 << root) == 0
            && let crate::tool::StringValue::Json { owner, range } = &value
            && owner.len() >= 64 * 1024
            && range.len() <= 4096
            && range.len() <= self.available()
        {
            // Validate/charge before copying. Keep transient root + leaf within
            // the existing budget; low headroom retains the original owner.
            let owner = Bytes::copy_from_slice(&owner[range.clone()]);
            let range = 0..owner.len();
            return Ok(crate::tool::StringValue::Json { owner, range });
        }
        Ok(value)
    }
    fn charge(&mut self, bytes: usize) -> Result<(), TaskError> {
        if bytes > self.scan_left {
            return Err(TaskError::Budget);
        }
        self.scan_left -= bytes;
        Ok(())
    }
    pub(crate) fn contains(&mut self, slot: usize, needle: usize) -> Result<i64, TaskError> {
        // Validate even an empty-needle haystack; charge each bounded search.
        let size = self.locate(slot)?.len() + self.plan.needles[needle].len();
        self.charge(size)?;
        let plan = self.plan.clone();
        let value = self.string(slot)?;
        Ok(i64::from(value.contains(plan.needles[needle].as_str())))
    }
    pub(crate) fn equals(&mut self, slot: usize, literal: usize) -> Result<i64, TaskError> {
        self.predicate(slot, literal, false)
    }
    pub(crate) fn starts_with(&mut self, slot: usize, literal: usize) -> Result<i64, TaskError> {
        self.predicate(slot, literal, true)
    }
    fn predicate(&mut self, slot: usize, literal: usize, prefix: bool) -> Result<i64, TaskError> {
        // Locate before charging, then validate even empty/unequal-length values.
        // UTF-8 equality preserves exact Unicode scalar equality, without normalization.
        let size = self.locate(slot)?.len() + self.plan.needles[literal].len();
        self.charge(size)?;
        let plan = self.plan.clone();
        let value = self.string(slot)?;
        Ok(i64::from(if prefix {
            value.starts_with(plan.needles[literal].as_str())
        } else {
            value == plan.needles[literal]
        }))
    }
}
/// Reborrow the interior of a validated immutable JSON string token. The range
/// was bounds-checked by locate and checked as an unescaped string by sonic.
unsafe fn validated_span(owner: &Bytes, range: Range<usize>) -> &str {
    unsafe { std::str::from_utf8_unchecked(&owner[range.start + 1..range.end - 1]) }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) unsafe extern "C" fn native_read(
    output: *mut crate::runtime::Output,
    slot: i64,
    result: *mut i64,
) -> i64 {
    let output = unsafe { &mut *output };
    let json = output.json.as_mut().expect("sealed JSON plan");
    let value = if slot < 0 {
        json.view((-slot - 1) as usize)
    } else {
        json.integer(slot as usize)
    };
    match value {
        Ok(value) => {
            unsafe { *result = value };
            0
        }
        Err(e) => {
            output.error = Some(e);
            1
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) unsafe extern "C" fn native_string(
    output: *mut crate::runtime::Output,
    slot: i64,
    needle: i64,
    result: *mut i64,
) -> i64 {
    let output = unsafe { &mut *output };
    let json = output.json.as_mut().expect("sealed JSON plan");
    // Sealed compiler selector: -1 validates, >=0 contains. Below -1,
    // -2 - 2*index is equality, -3 - 2*index is startswith.
    let value = if needle == -1 {
        json.validate_string(slot as usize)
    } else if needle < -1 {
        let selector = (-needle - 2) as usize;
        if selector & 1 == 0 {
            json.equals(slot as usize, selector / 2)
        } else {
            json.starts_with(slot as usize, selector / 2)
        }
    } else {
        json.contains(slot as usize, needle as usize)
    };
    match value {
        Ok(v) => {
            unsafe { *result = v };
            0
        }
        Err(e) => {
            output.error = Some(e);
            1
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) unsafe extern "C" fn native_print_string(
    output: *mut crate::runtime::Output,
    slot: i64,
) -> i64 {
    let output = unsafe { &mut *output };
    match output.append_json_string(slot as usize) {
        Ok(()) => 0,
        Err(e) => {
            output.error = Some(e);
            1
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) unsafe extern "C" fn native_release(output: *mut crate::runtime::Output, live: i64) {
    let output = unsafe { &mut *output };
    if let Some(json) = &mut output.json {
        json.release(live as u32);
    }
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::{State, StringCache};
    use crate::tool::{PathSlot, Plan};
    use bytes::Bytes;
    use std::rc::Rc;

    fn state() -> State {
        State::new(
            Rc::new(Plan {
                roots: 1,
                paths: vec![PathSlot {
                    root: 0,
                    parent: None,
                    key: "s".into(),
                }],
                needles: vec!["é".into()],
                string_paths: true,
                ..Plan::default()
            }),
            1024,
            1024,
        )
    }

    #[test]
    fn validated_span_is_reused_and_invalidated_with_its_owner() {
        let mut state = state();
        state
            .install(0, Bytes::from_static("{\"s\":\"hé\"}".as_bytes()), 1024)
            .unwrap();
        assert_eq!(state.contains(0, 0), Ok(1));
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Validated
        ));
        assert_eq!(state.contains(0, 0), Ok(1));
        assert_eq!(state.held(), "{\"s\":\"hé\"}".len());

        // A failed replacement must retain the old validated owner and range.
        assert_eq!(
            state.install(0, Bytes::from_static(b"{}"), 1),
            Err(crate::error::TaskError::Budget)
        );
        assert_eq!(state.string(0), Ok("hé"));
        state
            .install(0, Bytes::from_static(br#"{"s":"new"}"#), 1024)
            .unwrap();
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Unchecked
        ));
        assert_eq!(state.string(0), Ok("new"));
        state.release(0);
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Unchecked
        ));
        state
            .install(0, Bytes::from_static(br#"{"s":"\u0065"}"#), 1024)
            .unwrap();
        assert_eq!(state.string(0), Ok("e"));
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Decoded(_)
        ));
        state
            .install(0, Bytes::from_static(br#"{"s":""}"#), 1024)
            .unwrap();
        assert_eq!(state.string(0), Ok(""));
    }

    #[test]
    fn rejected_plain_token_never_sets_validation_marker() {
        let mut state = state();
        state
            .install(0, Bytes::from_static(b"{\"s\":\"bad\x01\"}"), 1024)
            .unwrap();
        assert!(matches!(
            state.string(0),
            Err(crate::error::TaskError::Json(_))
        ));
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Unchecked
        ));
        state
            .install(0, Bytes::from_static(b"{\"s\":\"\xff\"}"), 1024)
            .unwrap();
        assert!(matches!(
            state.string(0),
            Err(crate::error::TaskError::Json(_))
        ));
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Unchecked
        ));
    }

    #[test]
    fn argument_copy_cutoffs_headroom_and_scan_charge() {
        for (root_len, leaf_len, headroom, external, live, copied) in [
            (65536, 10, 10, 0, 0, true),
            (65535, 10, 10, 0, 0, false),
            (65536, 4096, 4096, 0, 0, true),
            (65536, 4097, 4097, 0, 0, false),
            (65536, 10, 9, 0, 0, false),
            (65536, 10, 10, 1, 0, false),
            (65536, 10, 10, 0, 1, false),
        ] {
            let text = "x".repeat(leaf_len);
            let mut raw = format!("{{\"s\":\"{text}\"}}").into_bytes();
            raw.resize(root_len, b' ');
            let mut state = state();
            state.max = root_len + headroom;
            state.scan_left = 10000;
            state.set_external(external);
            state.install(0, Bytes::from(raw), root_len).unwrap();
            let value = state.string_argument(0, live).unwrap();
            assert_eq!(value.as_str(), text);
            assert_eq!(state.scan_left, 10000 - leaf_len - 2);
            let crate::tool::StringValue::Json { owner, .. } = value else {
                panic!("plain string expected")
            };
            assert_eq!(owner.len(), if copied { leaf_len } else { root_len });
        }
    }

    #[test]
    fn get_found_and_absence_share_only_existing_cache_lifetime() {
        let mut state = state();
        state
            .install(0, Bytes::from_static(br#"{"s":"\u0078"}"#), 1024)
            .unwrap();
        assert_eq!(state.get_present(0), Ok(true));
        assert_eq!(state.string_value(0).unwrap().as_str(), "x");
        state.install(0, Bytes::from_static(b"{}"), 1024).unwrap();
        let before = state.scan_left;
        assert_eq!(state.get_present(0), Ok(false));
        assert_eq!(state.get_present(0), Ok(false));
        assert!(state.ranges[0].is_none());
        assert!(matches!(
            state.strings.as_ref().unwrap()[0],
            StringCache::Unchecked
        ));
        assert_eq!(state.scan_left, before);
        assert!(state.validate_string(0).is_err()); // required still missing
        state.release(0);
        assert!(state.get_present(0).is_err()); // released is not absence
        state
            .install(0, Bytes::from_static(br#"{"s":"new"}"#), 1024)
            .unwrap();
        assert_eq!(state.get_present(0), Ok(true));
        assert_eq!(state.string_value(0).unwrap().as_str(), "new");
    }

    #[test]
    fn static_json_task_state_size() {
        // The pre-string state was Rc<Plan> plus three Vecs (80 bytes).
        // Non-string JSON tasks do not allocate a decoded vector, but retain
        // the additional inline Option<Vec> and budget counters.
        assert_eq!(std::mem::size_of::<super::State>(), 128);
        // Ordinary escaped JSON still caches an inline String. A Tool argument
        // moves that string to an Arc only when it must outlive cancellation.
        assert_eq!(std::mem::size_of::<super::StringCache>(), 24);
    }
}
