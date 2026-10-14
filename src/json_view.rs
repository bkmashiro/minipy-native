//! Borrowed JSON access. No self-references, DOM, unchecked sonic APIs or user pointers.
use crate::{error::TaskError, tool::Plan};
use bytes::Bytes;
use std::ops::Range;

pub(crate) struct State {
    plan: std::rc::Rc<Plan>,
    roots: Vec<Option<Bytes>>,
    ranges: Vec<Option<Range<usize>>>,
    integers: Vec<Option<i64>>,
}
impl State {
    pub(crate) fn new(plan: std::rc::Rc<Plan>) -> Self {
        Self {
            roots: vec![None; plan.roots],
            ranges: vec![None; plan.paths.len()],
            integers: vec![None; plan.paths.len()],
            plan,
        }
    }
    pub(crate) fn held(&self) -> usize {
        self.roots.iter().flatten().map(Bytes::len).sum()
    }
    pub(crate) fn install(
        &mut self,
        root: usize,
        bytes: Bytes,
        max: usize,
    ) -> Result<(), TaskError> {
        if bytes.len() > max.saturating_sub(self.held()) {
            return Err(TaskError::Budget);
        }
        self.roots[root] = Some(bytes);
        Ok(())
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
            }
        }
    }
    pub(crate) fn view(&mut self, slot: usize) -> Result<i64, TaskError> {
        self.locate(slot).map(|_| 0)
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
        let raw = raw.as_raw_str().as_bytes();
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
        let range = offset..end;
        self.ranges[slot] = Some(range.clone());
        Ok(range)
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
pub(crate) unsafe extern "C" fn native_release(output: *mut crate::runtime::Output, live: i64) {
    let output = unsafe { &mut *output };
    if let Some(json) = &mut output.json {
        json.release(live as u32);
    }
}
