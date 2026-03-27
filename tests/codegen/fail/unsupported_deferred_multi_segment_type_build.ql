use Command as Cmd

struct Command {
    value: Int,
}

extern "c" pub fn q_accept(value: Cmd.Scope.Config) -> Int {
    return 0
}
