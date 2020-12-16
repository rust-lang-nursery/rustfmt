// rustfmt-unnest_imports: true

use a::{b::c, d::e};
use a::{f, g::{h, i}};
use a::{j::{self, k::{self, l}, m}, n::{o::p, q}};
pub use a::{r::s, t};

#[cfg(test)]
use foo::{a::b, c::d};

use bar::{
    // comment
    a::b,
    // more comment
    c::d,
    e::f,
};
