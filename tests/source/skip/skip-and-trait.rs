// *** "rustfmt::skip" and Trait
// *** All source lines starts with indent by 3 tabls.

// Trait with Attributs
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            trait Animal1 {
            // Static method signature; `Self` refers to the implementor type.
            fn new(name: &'static str) -> Self;

            // Traits can provide default method definitions.
            fn talk(&self) {
            println!("{} says {}", self.name(), self.noise());
            }
            }

            trait Animal2 {
            // Static method signature; `Self` refers to the implementor type.
            fn new(name: &'static str) -> Self;

            // Instance method signatures; these will return a string.
            fn name(&self) -> &'static str;
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            fn noise(&self) -> &'static str;

            fn talk(&self) {
            // Traits can provide default method definitions.
            #[rustfmt::skip]
            #[allow(non_snake_case)]
            fn name(&self) -> &'static str;
            println!("{} says {}", self.name(), self.noise());
            }
            }
