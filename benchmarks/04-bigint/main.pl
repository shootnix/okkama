use strict; use warnings;
# Точные целые: факториал 12000 и 150000-е число Фибоначчи (Math::BigInt).
use Math::BigInt;
my $f = Math::BigInt->new(1);
$f->bmul($_) for 1 .. 12000;
my ($a, $b) = (Math::BigInt->new(0), Math::BigInt->new(1));
($a, $b) = ($b, $a->copy->badd($b)) for 1 .. 150000;
my $digits = length("$f") + length("$a");
print $digits, " ", $f->copy->bmod(1000000007), " ", $a->copy->bmod(1000000007), "\n";
