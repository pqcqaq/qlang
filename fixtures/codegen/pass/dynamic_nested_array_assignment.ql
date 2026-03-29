fn write_cell(row: Int, col: Int) -> Int {
    var matrix = [[1, 2, 3], [4, 5, 6]]
    matrix[row][col] = 9
    return matrix[row][col]
}
