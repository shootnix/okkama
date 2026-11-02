//! Immutable range bounds; iteration counters and inclusive ends are exact.
use crate::{IntValue, integer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    start: IntValue,
    end: IntValue,
    inclusive: bool,
}
impl Range {
    pub fn new(start: IntValue, end: IntValue, inclusive: bool) -> Result<Self, &'static str> {
        if integer::compare(&start, &end).is_gt() {
            return Err("Range start must be less than or equal to range end");
        }
        Ok(Self {
            start,
            end,
            inclusive,
        })
    }
    pub fn start(&self) -> IntValue {
        self.start.clone()
    }
    pub fn end_exclusive(&self) -> IntValue {
        if self.inclusive {
            integer::add(&self.end, &IntValue::from_i64(1))
        } else {
            self.end.clone()
        }
    }
}
