// Based on issue #2055:
pub trait A {}
pub trait B {}
pub trait C {}
pub trait Foo1: A + C + B {}
pub trait Foo2:
    // A and C
    A + C + B
{
}
pub trait Foo3:
    /* A and C */
    A + C + B
{
}
pub trait Foo4: // A and C
    A + C + B
{
}
pub trait Foo5: /* A and C */ A + C + B {}
pub trait Foo6: /* A and C */ A + C + B {}
pub trait Foo7:
    A
    + C
    // and B
    + B
{
}
pub trait Foo8:
    // A and C
    A
    + C
    // and B
    + B
{
}

// Other cases
trait Person {
    fn name(&self) -> String;
}
/*comment1*/
trait Person {
    fn name(&self) -> String;
}
trait Student: /* comment1 */ Person /* comment2 */ {
    fn university(&self) -> String;
}
trait Programmer /* comment1 */ {
    fn fav_language(&self) -> String;
}
trait CompSciStudent1: /* comment1 */ Programmer + Student /* comment2 */ {
    fn git_username(&self) -> String;
}
trait CompSciStudent2:
    /* comment1 Longggggggggggggggggggggggggggggggggggggggggggggggggg */
    Programmer + Student /* comment2 */
{
    fn git_username(&self) -> String;
}
trait CompSciStudent3: // comment1
    Programmer + Student /* comment2 */
{
    fn git_username(&self) -> String;
}
trait CompSciStudent4: // comment1 Longgggggggggggggggggggggggggggggggggggggggggggggggggg
    Programmer + Student /* comment2 */
{
    fn git_username(&self) -> String;
}

// Comment before Ident
trait /* comment1 */ Person {
    fn fav_language(&self) -> String;
}
trait // comment1
Person {
    fn fav_language(&self) -> String;
}
trait /* comment 1 */ Programmer /* comment2 */ {
    fn fav_language(&self) -> String;
}
trait /* comment1 */ CompSciStudent1: /* comment2 */ Programmer + Student /* comment3 */ {
    fn git_username(&self) -> String;
}

// Traits with where and comments
trait Bar
where
    Self: Sized,
    Option<Self>: Foo,
{
}
/*comment0*/
trait Bar
/*comment1*/
where
    Self: Sized, /*comment2*/
    /*comment3*/ Option<Self>: Foo, /*comment4*/
{
}
trait Bar
//comment1 Longgggggggggggggggggggggggggggggggggggggggggggggggggg
where
    Self: Sized, /*comment2*/
    /*comment3*/ Option<Self>: Foo, /*comment4*/
{
}
trait Bar
/*comment1*/
where
    Self: Sized, /*comment2*/
    /*comment3*/ Option<Self>: Foo,
//comment4 Longgggggggggggggggggggggggggggggggggggggggggggggggggg
{
}
trait Bar
/*comment1 Longgggggggggggggggggggggggggggggggggggggggggggggggggg*/
where
    Self: Sized, /*comment2 Longgggggggggggggggggggggggggggggggggggggggggggggggggg*/
    /*comment3 Longgggggggggggggggggggggggggggggggggggggggggggggggggg*/ Option<Self>: Foo,
/*comment4 Longgggggggggggggggggggggggggggggggggggggggggggggggggg*/
{
}
trait ConstCheck<T>: /*comment1*/ Foo
where
    T: Baz, /*comment2*/
{
    const J: i32;
}

// Some other trait cases with comments
/*comment0*/
auto trait Example /*comment1*/ {}
pub unsafe auto trait PubUnsafeExample /*comment1*/ {}
pub unsafe auto trait PubUnsafeExample // comment1
{
}
trait Foo /*comment1*/ {
    type Bar: Baz;
    type Inner: Foo = Box<Foo>;
}
pub trait Iterator /*comment1*/ {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
pub trait Iterator //comment1
{
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}
