// #4027
pub use views::*;
pub use foo::bar; // re-export for frontend

// #3720
use c; // use c;
use b; // use b;
use a; // use a;

// #3127
extern crate serde; // 1.0.78
extern crate serde_json; // 1.0.27
extern crate serde_derive; // 1.0.78

mod c;
mod b;
mod a;

/*dpre*/ use d; // dpost
/*cpre*/ use c; // cpost
/*bpre*/ use b; // bpost
/*apre*/ use a; // apost

use std::{};
use std::borrow::Cow;

/* comment 1 */ use std::{};
/* comment 2 */ use std::{};
/* comment 3 */ use std::{};

use std::{}; /* comment 1 */
use std::{}; /* comment 2 */
use std::{}; /* comment 3 */

/* comment cpre */ use std::{}; /* comment cpost */
/* comment bpre */ use std::{}; /* comment bpost */
/* comment dpre */ use std::{}; /* comment dpost */

/* comment 1 */ use a;
/* comment 2 */ use b;
/* comment 3 */ use c;

use std::a; /* comment 1 */
use std::b; /* comment 2 */
use std::c; /* comment 3 */

/* comment cpre */ use std::c; /* comment cpost */
/* comment bpre */ use std::b; /* comment bpost */
/* comment dpre */ use std::a; /* comment dpost */


/* comment 1 */ use std::{}; // Comment 1
/* comment 2 */ use std::{}; // Comment 2
/* comment 3 */ use std::{}; // Comment 3


/* Comment
A */ use a; // Comment A
/* Comment B */ use b; // Comment B
/* Comment C */ use c; // Comment C
/* Comment D */ use d; /* Comment
D */
