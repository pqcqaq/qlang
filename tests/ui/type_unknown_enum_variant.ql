use Command as Cmd
use Result as Res

enum Command {
    Config {
        retries: Int,
    },
}

enum Result {
    Named {
        value: Int,
    },
    Value(Int),
    Empty,
}

fn main(command: Command, result: Result) -> Int {
    let direct_literal = Command.Missing { retries: 1 };
    let alias_literal = Cmd.Other { retries: 1 };
    let Result.Absent(tuple_value) = result;
    let Res.Unknown { value } = result;
    let Res.Gone = result;
    return 0
}
