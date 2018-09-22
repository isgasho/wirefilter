use lex::{Lex, LexResult};
use std::{borrow::Borrow, cmp::Ordering};
use strict_partial_ord::StrictPartialOrd;

/// Uninhabited / empty type for `bool` with traits we need for RHS values.
/// See https://doc.rust-lang.org/nomicon/exotic-sizes.html#empty-types.
#[derive(Debug, PartialEq, Eq, Clone, Hash, Serialize)]
pub enum UninhabitedBool {}

impl Borrow<bool> for UninhabitedBool {
    fn borrow(&self) -> &bool {
        match *self {}
    }
}

impl PartialEq<UninhabitedBool> for bool {
    fn eq(&self, other: &UninhabitedBool) -> bool {
        match *other {}
    }
}

impl PartialOrd<UninhabitedBool> for bool {
    fn partial_cmp(&self, other: &UninhabitedBool) -> Option<Ordering> {
        match *other {}
    }
}

impl StrictPartialOrd<UninhabitedBool> for bool {}

impl<'i> Lex<'i> for UninhabitedBool {
    fn lex(_input: &str) -> LexResult<Self> {
        unreachable!()
    }
}
