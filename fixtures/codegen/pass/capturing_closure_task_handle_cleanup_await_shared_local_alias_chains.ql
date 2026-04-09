extern "c" fn sink(value: Int)

async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let branch = true
    let first = spawn worker(41)
    let second = spawn worker(1)
    let left = () => first
    let right = () => second
    var alias_if = left
    var alias_match = left

    defer if await ({
        let chosen = if branch { alias_if = right } else { left }
        let rebound = chosen
        rebound
    })() == 2 {
        sink(1);
    }

    defer match await ({
        let chosen = match branch {
            true => alias_match = right,
            false => left,
        }
        let rebound = chosen
        rebound
    })() {
        2 => sink(2),
        _ => sink(3),
    }
    return 0
}
