use Point as P
use Result as Res

struct Point {
    x: Int,
}

enum Result {
    Named {
        value: Int,
    },
    Value(Int),
}

fn main(point: Point, result: Result) -> Int {
    let Point(value) = point;
    let P(alias_value) = point;
    let Result.Value { value: tuple_value } = result;
    let Res { value: root_value } = result;
    return 0
}
