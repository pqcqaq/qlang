use INPUT as DATA

struct Wrap {
    values: (Int, Int),
}

const INPUT: Wrap = Wrap { values: (20, 22) }

async fn main() -> Int {
    var total = 0
    for await value in DATA.values {
        total = total + value
    }
    return total
}
