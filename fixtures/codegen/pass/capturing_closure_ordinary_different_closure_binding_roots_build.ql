fn main() -> Int {
    let branch = true
    let target = 42
    let left = (value: Int) => value + target
    let right = (value: Int) => value + target + 1
    let chosen_if = if branch { left } else { right }
    let chosen_match = match branch {
        true => {
            let alias = left
            alias
        },
        false => right,
    }
    let alias_if = chosen_if
    let alias_match = chosen_match
    return alias_if(1) + alias_match(2)
}
