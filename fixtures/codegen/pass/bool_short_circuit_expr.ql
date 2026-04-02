fn left_true() -> Bool {
    return true
}

fn left_false() -> Bool {
    return false
}

fn right_true() -> Bool {
    return true
}

fn right_false() -> Bool {
    return false
}

fn main() -> Int {
    let both = left_false() && right_true()
    let either = left_true() || right_false()

    if both {
        return 1
    }

    if either && !both {
        return 3
    }

    return 4
}
