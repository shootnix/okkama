use strict; use warnings;
# Плавающая точка: множество Мандельброта 240x120, до 200 итераций.
package Mandel;
sub new { bless {}, shift }
sub escape {
    my ($self, $cr, $ci, $zr, $zi, $step) = @_;
    until ($step == 200 || $zr * $zr + $zi * $zi > 4.0) {
        ($zr, $zi) = ($zr * $zr - $zi * $zi + $cr, 2.0 * $zr * $zi + $ci);
        $step++;
    }
    return $step;
}
sub row {
    my ($self, $y) = @_;
    my $ci = $y * 0.02 - 1.2;
    my $total = 0;
    $total += $self->escape($_ * 0.0125 - 2.0, $ci, 0.0, 0.0, 0) for 0 .. 239;
    return $total;
}
package main;
my $mandel = Mandel->new;
my $sum = 0;
$sum += $mandel->row($_) for 0 .. 119;
print "$sum\n";
