struct Pair {
    left: Int,
    right: Int,
}

struct Packet {
    pair: Pair,
    values: [Int; 2],
}

async fn combine(pair: Pair, values: [Int; 2]) -> Int {
    return pair.left + pair.right + values[0] + values[1]
}

async fn nested(packet: Packet) -> Int {
    return packet.pair.left + packet.pair.right + packet.values[0] + packet.values[1]
}

async fn main() -> Int {
    let first = await combine(Pair { left: 1, right: 2 }, [3, 4])

    let second_task = spawn combine(Pair { left: 5, right: 6 }, [7, 8])
    let second = await second_task

    let third = await nested(Packet {
        pair: Pair { left: 9, right: 10 },
        values: [11, 12],
    })

    let fourth_task = spawn nested(Packet {
        pair: Pair { left: 13, right: 14 },
        values: [15, 16],
    })
    let fourth = await fourth_task

    return first + second + third + fourth
}
