fn main() -> Int {
let left = "alpha"
let right = "beta"
let prefix = "alpha"
let longer = "alphabet"

if left < right {
    if right > left {
        if prefix <= longer {
            if longer >= prefix {
                return 0
            }
        }
    }
}

return 1
}
