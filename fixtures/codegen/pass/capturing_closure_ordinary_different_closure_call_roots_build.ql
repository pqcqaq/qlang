fn main() -> Int {
    let branch = true
    let target = 42
    let left = (value: Int) => value + target
    let right = (value: Int) => value + target + 1
    return (if branch { left } else { right })(1)
        + (match branch {
            true => {
                let alias = left
                alias
            },
            false => right,
        })(2)
}
