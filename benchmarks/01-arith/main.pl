use strict; use warnings;
# Целочисленная арифметика в цикле: линейный конгруэнтный генератор.
sub run {
    my ($left, $state, $sum) = @_;
    while ($left) {
        $state = ($state * 1103515245 + 12345) % 2147483648;
        $sum += $state % 1000;
        $left--;
    }
    return $sum;
}
print run(3000000, 42, 0), "\n";
