// rustfmt-space_around_attr_eq: true

fn attr_eq_test() {
    #[cfg(not(target_os = "pi"))]
    println!("os is not pi!");
}
