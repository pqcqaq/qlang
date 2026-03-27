use Command as Cmd

enum Command {
    Config {
        retries: Int,
    },
    Value(Int),
}

fn main() -> Int {
    let builtin_value = Int { value: 1 };
    let enum_value = Cmd { retries: 1 };
    let tuple_variant = Command.Value { value: 1 };
    return 0
}
