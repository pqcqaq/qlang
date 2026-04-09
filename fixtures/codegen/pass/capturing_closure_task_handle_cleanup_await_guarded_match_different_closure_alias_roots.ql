extern "c" fn choose() -> Bool
extern "c" fn guard() -> Bool
extern "c" fn sink(value: Int)

async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let branch = choose()
    let first = spawn worker(41)
    let second = spawn worker(1)
    let left = () => first
    let right = () => second

    defer if await ({
        let chosen = match branch {
            true if guard() => left,
            false => right,
            _ => right,
        }
        let rebound = chosen
        rebound
    })() == 42 {
        sink(1);
    }

    defer match await ({
        let chosen = match branch {
            true if guard() => {
                let alias = left
                alias
            },
            false => right,
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
