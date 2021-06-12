// *** "rustfmt::skip" and Function
// *** All source lines starts with indent by 3 tabls.

// "rustfmt::skip" as first line;
            #[rustfmt::skip]
            /** Comment line 1
            comment line 2. */
            fn main() {
            x = y;
            }

// "rustfmt::skip" whithin a block of attributes
            fn main() {
            #[rustfmt::skip]
            // Comment 1
            #[allow(non_snake_case)]
            // Comment 2
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            fn main() {
            // Comment 1
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            // Comment 2
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            fn main() {
            // Comment 1
            #[allow(non_snake_case)]
            // Comment 2
            #[allow(non_snake_case)]
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            fn main() {
            #[rustfmt::skip]
            /** Comment line 1
            comment line 2. */
            #[allow(non_snake_case)]
            x = y;
            }

// fn with Doc comment which is attribute vs one-line comment
            fn main() {
            // Comment for `foo`
            #[rustfmt::skip] // comment on why use a skip here
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }
