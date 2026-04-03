use INDEX as SLOT

const INDEX: Int = 0
static NEXT: Int = 1

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pair = (1, 2)
    let first = pair[SLOT] = await worker(8)
    let second = pair[NEXT] = first + 6
    return first + second
}
