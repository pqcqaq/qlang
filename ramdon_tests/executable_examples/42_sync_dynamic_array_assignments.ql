fn score(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    var index = 1
    var values = [3, 4, 5]
    values[index] = values[0] + values[2]

    var row = 1
    var col = 0
    var matrix = [[1, 2], [3, 4]]
    matrix[row][col] = values[index] + matrix[0][1]

    return score(values[1], matrix[1][0])
}
