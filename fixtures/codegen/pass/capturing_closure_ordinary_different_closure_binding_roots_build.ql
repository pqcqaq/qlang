fn run_if() -> Int {
    let branch = true
    let target = 42
    let left = (value: Int) => value + target
    let right = (value: Int) => value + target + 1
    let chosen = if branch { left } else { right }
    return chosen(1)
}

fn run_match() -> Int {
    let branch = true
    let target = 42
    let left = (value: Int) => value + target
    let right = (value: Int) => value + target + 1
    let chosen = match branch {
        true => {
            let alias = left
            alias
        },
        false => right,
    }
    return chosen(2)
}

fn main() -> Int {
    return run_if() + run_match()
}
