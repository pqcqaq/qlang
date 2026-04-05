fn main() -> Int {
    let branch = true
    let target = 42
    let left = (value: Int) => value == target
    let right = (value: Int) => value + 1 == target + 1
    let first = match 42 {
        current if ({
            let chosen = if branch { left } else { right }
            chosen
        })(current) => 1,
        _ => 0,
    }
    let second = match 42 {
        current if ({
            let chosen = match branch {
                true => {
                    let alias = left
                    alias
                },
                false => right,
            }
            let alias = chosen
            alias
        })(current) => 2,
        _ => 0,
    }
    return first + second
}
