// #4027
pub use foo::bar; // re-export for frontend
pub use views::*;

// #3720
use a; // use a;
use b; // use b;
use c; // use c;

// #3127
extern crate serde; // 1.0.78
extern crate serde_derive; // 1.0.78
extern crate serde_json; // 1.0.27

mod a;
mod b;
mod c;

/*apre*/ use a; // apost
/*bpre*/ use b; // bpost
/*cpre*/ use c; // cpost
/*dpre*/ use d; // dpost

use std::borrow::Cow;

/* comment 1 */
/* comment 2 */
/* comment 3 */

/* comment 1 */
/* comment 2 */
/* comment 3 */

/* comment cpre */
/* comment cpost */
/* comment bpre */
/* comment bpost */
/* comment dpre */
/* comment dpost */

/* comment 1 */ use a;
/* comment 2 */ use b;
/* comment 3 */ use c;

use std::a; /* comment 1 */
use std::b; /* comment 2 */
use std::c; /* comment 3 */

/* comment dpre */ use std::a; /* comment dpost */
/* comment bpre */ use std::b; /* comment bpost */
/* comment cpre */ use std::c; /* comment cpost */

/* comment 1 */
// Comment 1
/* comment 2 */
// Comment 2
/* comment 3 */
// Comment 3

/* Comment
A */
use a; // Comment A
/* Comment B */ use b; // Comment B
/* Comment C */ use c; // Comment C
/* Comment D */ use d; /* Comment
                       D */
