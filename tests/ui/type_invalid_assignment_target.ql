use limit as imported_limit

const limit: Int = 1
static total: Int = 0

fn add(left: Int, right: Int) -> Int {
    return left + right
}

struct Counter {
    value: Int,
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    var values = [1, 2, 3]
    limit = 2;
    total = 3;
    add = add;
    imported_limit = 4;
    counter.value = 5;
    values[0] = 6
    return values[0]
}
