fn main() -> Int {
    let branch = true
    let target = 42
    let check = (value: Int) => value == target
    let first = match 42 {
        current if ({
            var alias = check
            let chosen = alias = check
            chosen
        })(current) => 1,
        _ => 0,
    }
    let second = match 42 {
        current if ({
            var alias = check
            let chosen = if branch { alias = check } else { check }
            chosen
        })(current) => 2,
        _ => 0,
    }
    return first + second
}
