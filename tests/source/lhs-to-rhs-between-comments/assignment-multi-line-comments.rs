// "=" Assignemnt - Multiline Block comments
fn main () {
let var = /* Block comment line 1 
* Block comment line 2
* Block comment line 3 */ first;
var = /* Block comment line 1 
* Block comment line 2
* Block comment line 3 */ second;
var = /* Block comment line 1 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 2
* Block comment line 3 */ third;
var = /* Block comment line 1 
* Block comment line 2 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 3 */ forth;
}

// "=" Assignemnt - Multiline Line and Block comments
fn main () {
let var = // Line comment 1
// Line comment 2
// Line comment 3
first;
var = // Line comment 
/* Block comment line 1
* Block comment line 2 */ second;
var = /* Block comment single line */ 
// Line comment
/* Block comment line 1
* Block comment line 2 */ third;
var = /* Block comment line 1 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 2 */
// Line comment
forth;
var = // Line comment 
/* Block comment line 1 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 2 */ fifth;
}

// BinOp Assignemnt - Multiline Block comments
fn main () {
let var = /* Block comment line 1 
* Block comment line 2
* Block comment line 3 */ first;
var += /* Block comment line 1 
* Block comment line 2
* Block comment line 3 */ second;
var -= /* Block comment line 1 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 2
* Block comment line 3 */ third;
var *= /* Block comment line 1 
* Block comment line 2 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 3 */ forth;
}

// BinOp Assignemnt - Multiline Line and Block comments
fn main () {
let var = // Line comment 1
// Line comment 2
// Line comment 3
first;
var += // Line comment 
/* Block comment line 1
* Block comment line 2 */ second;
var -= /* Block comment single line */ 
// Line comment
/* Block comment line 1
* Block comment line 2 */ third;
var *= /* Block comment line 1 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 2 */
// Line comment
forth;
var /= // Line comment 
/* Block comment line 1 longggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
* Block comment line 2 */ fifth;
}
