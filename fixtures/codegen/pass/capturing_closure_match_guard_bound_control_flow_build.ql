fn main() -> Int {
    let branch = true
    let target = 42
    let check = (value: Int) => value == target
    var alias = check
    let chosen = if branch {
        let local = check
        local
    } else {
        check
    }
    let rebound = match branch {
        true => alias = check,
        false => check,
    }
    let first = match 42 {
        current if chosen(current) => 1,
        _ => 0,
    }
    let second = match 42 {
        current if rebound(current) => 2,
        _ => 0,
    }
    return first + second
}
