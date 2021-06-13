// Tests for original #3943 issue
use ::foo;
use ::foo::{Bar1};
use ::foo::{Bar2, Baz2};
use ::{Foo};
use ::{Bar3, Baz3};

use ::foo;
use ::foo::Bar;
use ::foo::{Bar, Baz};
use ::Foo;
use ::{Bar, Baz};

use ::Foo;
use ::foo;
use ::foo::Bar;
use ::foo::{Bar, Baz};
use ::{Bar, Baz};

// Additional tests for signle item `{}` handling
use ::AAAA;
use bbbbb::AAAA;
use ::{BBBB};
use aaaa::{BBBB};
use crate::detect::{Feature, cache};
use super::{auxvec};

// Tests with comments
use a::{/* pre-comment */ item};
use a::{ item  /* post-comment */};
use a::{/* pre-comment */ item    /* post-comment */   };

// Misc
use ::{Foo};
use ::foo;
use ::{Foo1};
use std;
use dummy;
use Super::foo;
use ::*;
use ::foo::{Foo, foo};
use self::std::fs as self_fs;
use ::foo as bar;
use ::{Foo as baz};
