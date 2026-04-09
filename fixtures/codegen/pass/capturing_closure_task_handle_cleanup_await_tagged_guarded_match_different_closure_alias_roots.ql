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

    defer if await ({
        let chosen = match key {
            current if current == 42 => left,
            _ => right,
        }
        let rebound = chosen
        rebound
    })() == 42 {
        sink(1);
    }

    defer match await ({
        let chosen = match key {
            current if current == 42 => {
                let alias = left
                alias
            },
            _ => right,
        }
        let rebound = chosen
        rebound
    })() {
        42 => sink(2),
        _ => sink(3),
    }
    return 0
}
