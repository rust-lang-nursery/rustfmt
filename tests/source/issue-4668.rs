// rustfmt-version: Two

const C: usize = 0_usize;
const U: usize = /* A long block-style comment A long block-style comment A long block-style comment A long block-style comment */ if C > 0 { 4 } else { 3 };

fn f() {
    fn g() -> int {
        let foo = 12;
        match foo {
            0..=10 => {
                /* Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin urna felis, molestie ex. */
            }
            _ => {
                let st = None;
                let res = st.unwrap_or(
                    /* Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin urna felis, molestie ex. */ "abc"
                );
            }
        }
    }
}

fn h() {
    let mut y = 0;
    while let x = vec![1, 2, 3]
        .iter()
        /* Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin urna felis, molestie ex... */
        .take(3)
        .skip(1)
        .to_owned()
        .next()
    {
        y += x.unwrap_or_else(|/* Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin urna felis, molestie ex... */_| &0) * 2;
    }
}

pub fn t(x: int) {
    if x == 3 {
        let y = match context {
            True => {
                |x| /* Lorem ipsum dolor sit amet, consectetur adipiscing elit. Proin urna felis, molestie ex... */ x+2
            }
            False => |x| x,
        };
    } else {}
}
