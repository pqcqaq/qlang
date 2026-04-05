fn main() -> Int {
    let branch = true
    let target = 42
    let check = (value: Int) => value == target
    let first = match 42 {
        current if (match branch {
            true => {
                let alias = check
                alias
            },
            false => check,
        })(current) => 1,
        _ => 0,
    }
    let second = match 42 {
        current if (if branch {
            var alias = check
            alias = check;
            alias
        } else {
            check
        })(current) => 2,
        _ => 0,
    }
    return first + second
}
