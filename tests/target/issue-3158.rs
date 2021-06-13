// rustfmt-format_code_in_doc_comments: true

// Rust and ignore or compile_fail attributes:

/// ```rust
/// fn main() {
///     x = y;
/// }
/// ```
foo! { x, y }

/// ```rust,ignore
/// fn main() {
/// x = y;
/// }
/// ```
foo! { x, y }

/// ```rust,no_run
/// fn main() {
///     x = y;
/// }
/// ```
foo! { x, y }

/// ```rust,   ignore  , no_run
/// fn main() {
/// x = y;
/// }
/// ```
foo! { x, y }

/// ```complie_fail
/// fn main() {
/// x = y;
/// }
/// ```
foo! { x, y }

/// ```rust, no_run,compile_fail
/// fn main() {
/// x = y;
/// }
/// ```
foo! { x, y }

/// ```
/// fn main() {
///     x = y;
/// }
/// ```
foo! { x, y }

/// ```,
/// fn main() {
///     x = y;
/// }
/// ```
foo! { x, y }

/// ```,,,,,,,,,,,
/// fn main() {
///     x = y;
/// }
/// ```
foo! { x, y }

/// ```,, ,    ,     rust    , , ,, , , ,
/// fn main() {
///     x = y;
/// }
/// ```
foo! { x, y }

/// ```,, ,    ,     rust    , , ,, ignore, , ,
/// fn main() {
/// x = y;
/// }
/// ```
foo! { x, y }
