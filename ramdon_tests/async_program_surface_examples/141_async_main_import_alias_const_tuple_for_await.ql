use VALUES as INPUT

const VALUES: (Int, Int) = (20, 22)

async fn main() -> Int {
    var total = 0
    for await value in INPUT {
        total = total + value
    }
    return total
}
