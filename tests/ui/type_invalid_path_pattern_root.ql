use Point as P
use Result as Res

struct Point {
    x: Int,
}

enum Result {
    Empty,
    Value(Int),
    Named {
        value: Int,
    },
}

fn main(point: Point, result: Result) -> Int {
    let Point = point;
    let P = point;
    let Result.Value = result;
    let Res.Named = result;
    return 0
}
