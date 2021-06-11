// rustfmt-control_brace_style: ClosingNextLine

fn main() {
    if false {
        let foo = ();
        let bar = ();
    }

    if false // lone if comment
    {
        let foo = ();
        let bar = ();
    }


    let a = if 0 > 1 { unreachable!() } else { 0x0 };


    if true {
        let foo = ();
    }
    else if false {
        let foo = ();
        let bar = ();
    }
    else {
        let foo = ();
        let bar = ();
        let baz = ();
    }

    if true // else-if-chain if comment
    {
        let foo = ();
    }
    else if false // else-if-chain else-if comment
    {
        let foo = ();
        let bar = ();
    }
    else // else-if-chain else comment
    {
        let foo = ();
        let bar = ();
        let baz = ();
    }
}
