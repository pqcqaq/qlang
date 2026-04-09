extern "c" fn sink(value: Int)

async fn worker(value: Int) -> Int {
    return value + 1
}

async fn main() -> Int {
    let branch = true
    let which = 1
    let first = spawn worker(41)
    let second = spawn worker(1)
    let left = () => first
    let right = () => second

    defer if await (if branch { left } else { right })() == 42 {
        sink(1);
    }
    defer match await (match which {
        1 => {
            let alias = left
            alias
        },
        _ => right,
    })() {
        42 => sink(2),
        _ => sink(3),
    }
    return 0
}
