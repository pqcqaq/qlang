extern "c" fn sink(value: Int)

async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let key = 42
    let first = spawn worker(41)
    let second = spawn worker(1)
    let left = () => first
    let right = () => second
    var alias_if = left
    var alias_match = left

    defer if await ({
        let chosen = match key {
            current if current == 42 => alias_if = right,
            _ => left,
        }
        let rebound = chosen
        rebound
    })() == 2 {
        sink(1);
    }

    defer match await ({
        let chosen = match key {
            current if current == 42 => alias_match = right,
            _ => left,
        }
        let rebound = chosen
        rebound
    })() {
        2 => sink(2),
        _ => sink(3),
    }
    return 0
}
