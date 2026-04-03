async fn worker(value: Int) -> Int {
    return value
}

fn score(left: Int, right: Int) -> Int {
    return left + right
}

async fn main() -> Int {
    var index = 1
    var tasks = [worker(3), worker(4)]
    tasks[index] = worker(8)

    let left = await tasks[0]
    let right = await tasks[1]

    return score(left, right)
}
