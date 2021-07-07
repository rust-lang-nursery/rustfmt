// Tests fns without function body

pub fn foo(a: AAA, b: BBB) -> RetType;

pub(crate) fn foo(a: AAA, b: BBB) -> RetType;

impl Foo {
    pub fn foo(a: AAA);
}
