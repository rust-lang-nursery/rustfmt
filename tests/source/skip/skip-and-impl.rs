// *** "rustfmt::skip" and Impl

// Original code from issue #4499
#[rustfmt::skip]
			/** Comment.

    Rustfmt skips this as expected. */
					fn main() {
				for _ in 1..Foo::getRandomNumber() {
        println!("Hello, world!");
			}
}

struct Foo;
				impl Foo {
            #[rustfmt::skip]
				/** Comment.

        Rustfmt unindents this despite the ::skip above. */
			#[allow(non_snake_case)]
				pub fn getRandomNumber() -> i32 { 4 }
}

// *** All source lines starts with indent by 3 tabls.

// "rustfmt::skip" as first line;
            #[rustfmt::skip]
            /** Comment line 1
            comment line 2. */
            #[allow(non_snake_case)]
            impl Foo {
            pub fn getRandomNumber() -> i32 { 4 }
            }

// "rustfmt::skip" whithin a block of attributes
            impl Foo {
            #[rustfmt::skip]
            // Comment 1
            #[allow(non_snake_case)]
            // Comment 2
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            impl Foo {
            // Comment 1
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            // Comment 2
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            impl Foo {
            // Comment 1
            #[allow(non_snake_case)]
            // Comment 2
            #[allow(non_snake_case)]
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            impl Foo {
            #[rustfmt::skip]
            /** Comment line 1
            comment line 2. */
            #[allow(non_snake_case)]
            pub fn getRandomNumber() -> i32 { 4 }
            }

// impl with Doc comment which is attribute vs one-line comment
            impl Struct {
            /// Documentation for `foo`
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

            impl Struct {
            // Comment for `foo`
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            pub fn foo(&self) {}
            }

