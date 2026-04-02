use make_handle as run

fn make_handle(value: Int) -> Task[Int] {
    return worker(value)
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var total = 0
    for await task in [run(value: 20), run(value: 22)] {
        total = total + await task
    }
    return total
}
