// Issue #4549

// Different `if`/`else` expressions
// NOTE: comments before and after the condition sign are currently not handled,
//       so the original code snippet is used.
//       Therefore, althorugh  such cases are included in some of the tests,
//       such as `if x /*x*/ == /*y*/ y`, they are not really part of the tests purpose.
fn main() {
    if /*w*/ x /*x*/ == /*y*/ y /*z*/ {}
}

fn main() {
    /*pre-if*/ if /*post-if*/ x == y /*pre-if-block*/ { x = y +7; }
	/*pre-elseif*/ else if /*post-elseif*/ z == u /*pre-elseif-block*/ { y = 5;}
	/*pre-else*/ else /*post-else*/ {z = x;}
}

fn main() {
    /*pre-if*/ if /*post-if*/ x /*x*/ == /*y*/ y /*pre-if-block*/ { x = y +7; }
	/*pre-elseif*/ else if /*post-elseif Longgggggggggggggggggggggggggggggggggggggggggggggggggggggggggg*/ z == u /*pre-elseif-block*/ { y = 5; }
	/*pre-else*/ else /*post-else*/ { z = x; }
}

fn main() {
    /*pre-if*/ if // post-if - make sure that x and y states are the same
	x == y //pre-if-block - when they are same change y to next state
	{ x = y +7; }
}

fn main() {
    /*pre-if*/ if // post-if - make sure that x and y states are the sameeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	x /*x*/ == /*y*/ y //pre-if-block - when they are same change y to next stateeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	{ x = y +7; }
}

fn main() {
    if 5 == 6 { x = y +7; }
	else if z == u { y = 5; }
	else { z = x; }
}

fn main() {
if // if comment
5 == 6 { x = y +7; }
}fn main() {
if
// if comment
5 == 6 { x = y +7; }
}

// Tests for `loop`
fn main() {
				loop                {           x = y +7;              };

	/*pre-statement*/ loop /*pre-block*/ { x = y +7; };

	/*pre-statement*/ loop /*pre-block Longgggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg*/ { y = 5; };

	/*pre-statement*/ loop /*pre-block Longerrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr*/ { y = 5; };

	loop /*pre-block Longgggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg*/ {exppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp};		
}

// Tests for `while`
// NOTE: same coments limitations as for `if`.
fn main() {
	/*pre-statement*/ while /*post-cond*/ x /*x*/ == /*y*/ y /*pre-block*/ { x = y +7; };

	/*pre-statement*/ while /*pre-cond Longgggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg*/ z == u /*pre-block*/ { y = 5; };

	/*pre-statement*/ while /*pre-cond Longerrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr*/ z == u /*pre-block*/ { y = 5; };

	/*pre-statement*/ while // pre-cond - make sure that x and y states are the sameeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	x /*x*/ == /*y*/ y //pre-block - when they are same change y to next stateeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	{ x = y +7; }
}

// Tests for `if/while let`
// NOTE: comments before an after the `pat` item that follows `let` (e.g. the `Some(x)`
//       in `let Some(x) =`) are currently not handledand, so original code is used.
//       E.g., in `while let /*C1*/ Some(x) /*C2*/ = /*C3*/ exp /*C4*/{}`, only the
//       `C3` and `C4` comments are handled, and therefore comments such as `C1` AND `C2`
//       are not included in the test.
fn main() {
	/*pre-statement*/ while let Some(x) = /*pre-epr*/ exp /*pre-block*/ { y = x +7; };

	/*pre-statement*/ while let Some(x) = /*pre-expr Longggggggggggggggggggggggggggggggggggggggggggggggggggg*/ exp /*pre-block*/ { y = x; };

	/*pre-statement*/ while let Some(x) = /*pre-expr Longerrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr*/ exp /*pre-block*/ { y = x; };

	/*pre-statement*/ while let Some(x) = // pre-exprrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
	exp //pre-blockkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk
	{ y = x +7; }
}
fn main() {
	/*pre-statement*/ if let Some(x) = /*pre-epr*/ exp /*pre-block*/ { y = x +7; };

	/*pre-statement*/ if let Some(ref         /*def*/ mut       /*abc*/ state) =       /*pre-epr*/ exp           /*pre-block*/       { y = x +7; };

	/*pre-statement*/ if let Some(ref         /*def*/ mut       /*abc*/ state) =       /*pre-eprrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr*/ exp           /*pre-blockkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk*/         { y = x +7; };
}

// Tests for `for`
// NOTE: comments `in` aare currently not handledand, so original code is used.
//       E.g., in `for x /*C1*/ in /*C2*/`, only the `C2` comment is handled,
//       and therefore comments such as `C1` are not included in the test.
fn main() {
	/*pre-statement*/ for /*post-cond*/ x in /*post-in*/ exp /*pre-block*/ { y = x +7; };

	/*pre-statement*/for /*pre-cond Longggggggggggggggggggggggggggggggggggggggggggggggggggg*/ x in exp /*pre-block*/ { y = x; };

	/*pre-statement*/ for /*pre-cond Longerrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr*/ x in exp /*pre-block*/ { y = x; };

	/*pre-statement*/ for // pre-cond - make sure that x and y states are the sameeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	x in /*post-innnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn*/ exp //pre-block - when they are same change y to next stateeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	{ y = x +7; }
}
