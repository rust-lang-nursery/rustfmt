// *** "rustfmt::skip" and Whitespaces at nd of line
// *** All source lines starts with indent by 3 tabls.

// Impl - original code from issue #4706
impl Foo {
    #[rustfmt::skip]
            fn foo() {
            Bar     
            //     ^ there is whitespace here
            }

    fn bar() {
        Qux // asdf
    }
}

// Impl - all skipped lines end with white spaces
#[rustfmt::skip]
            /// DOC1   
            /// DOC2   
            impl      Foo1          {     
            fn foo1() {
            Bar1     
            }   
            }

impl Foo2 {
    #[rustfmt::skip]
            /// DOC1   
            /// DOC2   
            fn foo2() {    
            Bar2     
            }
}

impl Foo3 {
    fn foo3() {
        #[rustfmt::skip]
            /// DOC1   
            /// DOC2        
            Bar2
    }
}

// fn - all skipped lines end with white spaces
#[rustfmt::skip]
            /// DOC1   
            /// DOC2   
            fn foo2() {    
            Bar2     
            }

fn foo3() {
    #[rustfmt::skip]
            /// DOC1   
            /// DOC2        
            Bar2
}

// Trait - all skipped lines end with white spaces

// All skipped lines end with white spaces
#[rustfmt::skip]
            #[allow(non_snake_case)]    
            trait Animal1 {    
            fn new(name: &'static str) -> Self;    

            fn talk(&self) {}     
            }

// All skipped lines end with white spaces
#[allow(non_snake_case)]
trait Animal1 {
    #[rustfmt::skip]
            fn new(name: &'static str) -> Self;

    fn talk(&self) {}
}

// Internal skipped line
#[allow(non_snake_case)]
trait Animal3 {
    fn new(name: &'static str) -> Self;

    fn talk(&self) {
        #[rustfmt::skip]
            let x = 1;
        //          ^ there is whitespace here
    }
}

// Macro - all skipped lines end with white spaces.
#[rustfmt::skip]
            macro_rules! my_macro1 {    
            () => {};    
            }

// Skipped range in macro definitin body does **NOT** enter into final `skipped_range`
// list since macro definition is formatted as a stand alone `snippet`.
macro_rules! my_macro2 {
    ($param) => {
        #[rustfmt::skip]
            $param
        //     ^ there are **NO** trailing whitespaces here
    };
}
