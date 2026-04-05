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
    return chosen_if(1) + chosen_match(2)
}
