fn main() {
    loop {
        let foo = ();
        let bar = ();
    }


    'label: loop // loop comment
    {
        let foo = ();
    }


    cond = true;
    while cond {
        let foo = ();
    }


    'while_label: while cond {
        // while comment
        let foo = ();
    }


    for obj in iter {
        for sub_obj in obj {
            'nested_while_label: while cond {
                let foo = ();
            }
        }
    }

    match some_var {
        // match comment
        pattern0 => val0,
        pattern1 => val1,
        pattern2 | pattern3 => {
            do_stuff();
            val2
        }
    };
}
