trait Bar {
    fn bar(&self, a: T)
    where
        //     Self: Bar
        // Some comment
    ;

    fn bar2(&self, a: T)
    where
        /* Self: Bar */;
}
