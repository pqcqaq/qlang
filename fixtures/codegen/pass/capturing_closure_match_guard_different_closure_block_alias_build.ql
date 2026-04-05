fn main() -> Int {
    let branch = true
    let target = 42
    let left = (value: Int) => value == target
    let right = (value: Int) => value + 1 == target + 1
    let first = match 42 {
        current if (if branch {
            let alias = left
            alias
        } else {
            right
        })(current) => 1,
        _ => 0,
    }
    let second = match 42 {
        current if (match branch {
            true => {
                var alias = left
                alias = left;
                alias
            },
            false => right,
        })(current) => 2,
        _ => 0,
    }
    return first + second
}
