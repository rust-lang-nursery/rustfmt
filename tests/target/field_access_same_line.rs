// rustfmt-field_access_same_line: true

fn main() {
    variable.field.field
        .function()
        .field
        .function()
        .field.0.field
        .function()
        .0
        .function()
        .function()
        .0 .0 .0 .0 .0 .0.field.field.field.field.field.field.field.field.field.field.field.field
        .field // test
        .field.field.field // test
        .field.field.field // test
        // test
        .function()
        .field.0.field[5][6]
        .1.test[3];
}
