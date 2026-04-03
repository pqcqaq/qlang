struct State {
    value: Int,
}

async fn fetch_value(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await fetch_value(value: 22)
    let from_tuple_projection = match first {
        current if (0, current)[1] == 22 => 10,
        _ => 0,
    }

    let second = await fetch_value(value: 22)
    let from_struct_projection = match second {
        current if State { value: current }.value == 22 => 12,
        _ => 0,
    }

    let third = await fetch_value(value: 3)
    let from_array_projection = match third {
        current if [current, current + 1, current + 2][1] == 4 => 20,
        _ => 0,
    }

    return from_tuple_projection + from_struct_projection + from_array_projection
}
