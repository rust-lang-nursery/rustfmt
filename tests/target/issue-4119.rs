fn foo() -> ReturnCode {
    match command_num {
        0 /* check if present */ => ReturnCode::SUCCESS,
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        0 /* check if present */ if 1 = 0 => ReturnCode::SUCCESS,
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        0 if 1 = 0 /* check if present */ => ReturnCode::SUCCESS,
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        0 if /* check if present */ 1 = 0 => ReturnCode::SUCCESS,
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

// With long comments

fn foo() -> ReturnCode {
    match command_num {
        0 /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */ => {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        0 /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */ if 1 = 0 => {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        0 if 1 = 0 /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */
        => ReturnCode::SUCCESS,
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        0 if /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */ 1 = 0 => {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

// Multiline Pattern

fn foo() -> ReturnCode {
    match command_num {
        AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
        | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
        /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */ => {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
        | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
        /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */
        if 1 = 0 =>
        {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
        | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
            if 1 = 0 /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */
        =>
        {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}

fn foo() -> ReturnCode {
    match command_num {
        AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
        | BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
            if
            /* Loooooooooooooooooooooooooooooooooooooooooonnnnnnnnnnng comment */
            1 = 0 =>
        {
            ReturnCode::SUCCESS
        }
        // Check is sensor is correctly connected
        1 => {
            let x = 0;
            ReturnCode::EBUSY
        }
        _ => ReturnCode::ENOSUPPORT,
    }
}
