package demo.control

use std.stream.Stream

enum Command {
    Quit,
    Retry(Int),
    Pair(Int, Int),
    Config { enabled: Bool, retries: Int },
}

fn classify(command: Command, stream: Stream) -> Int {
    var total = 0

    while total < 10 {
        total = total + 1
    }

    loop {
        tick();
        break
    }

    for item in [1, 2, 3] {
        if item == 2 {
            continue
        };
        total = total + item
    }

    for await event in stream {
        total = total + event
    }

    let next = if total > 3 {
        match command {
            Command.Quit => 0,
            Command.Retry(times) if times > 1 => times,
            Command.Pair(left, right) => left + right,
            Command.Config { enabled: true, retries, .. } => retries,
            _ => total,
        }
    } else {
        total
    }

    return next
}
