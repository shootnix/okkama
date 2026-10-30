# Целочисленная арифметика в цикле: линейный конгруэнтный генератор.
def run(left, state, total):
    while left:
        state = (state * 1103515245 + 12345) % 2147483648
        total += state % 1000
        left -= 1
    return total


print(run(3000000, 42, 0))
