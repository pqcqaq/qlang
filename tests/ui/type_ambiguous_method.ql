struct Counter {
    value: Int,
}

extend Counter {
    fn ping(self) -> Int {
        return self.value
    }
}

extend Counter {
    fn ping(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.ping()
}
