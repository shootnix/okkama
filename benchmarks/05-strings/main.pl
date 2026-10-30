use strict; use warnings;
# Строки: сборка CSV на 30000 строк, split, замены, длины и разбор чисел.
sub line { my $i = shift; "id=$i;name=item-" . ($i * 7 % 1000) . ";price=" . ($i * 13 % 9973) }
sub price { my $f = (split /;/, shift)[2]; $f =~ s/price=//g; 0 + $f }
my $text = join "\n", map { line($_) } 0 .. 29999;
my @lines = split /\n/, $text;
my $total = 0;
$total += price($_) for @lines;
(my $renamed = $text) =~ s/item-/product_/g;
print scalar(@lines), " $total ", length($renamed), " ", length($text), "\n";
