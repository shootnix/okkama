use strict; use warnings;
# Роли: полиморфный вызов над 300000 фигурами четырёх видов.
package Rect;     sub new { bless { w => $_[1], h => $_[2] }, $_[0] }
sub area { $_[0]{w} * $_[0]{h} } sub perimeter { 2 * ($_[0]{w} + $_[0]{h}) } sub weight { 1 }
package Square;   sub new { bless { side => $_[1] }, $_[0] }
sub area { $_[0]{side} * $_[0]{side} } sub perimeter { 4 * $_[0]{side} } sub weight { 2 }
package Triangle; sub new { bless { a => $_[1], b => $_[2], c => $_[3] }, $_[0] }
sub area { $_[0]{a} * $_[0]{b} % 1000 } sub perimeter { $_[0]{a} + $_[0]{b} + $_[0]{c} } sub weight { 3 }
package Circle;   sub new { bless { r => $_[1] }, $_[0] }
sub area { 314 * $_[0]{r} * $_[0]{r} % 100000 } sub perimeter { 628 * $_[0]{r} % 10000 } sub weight { 4 }
package main;
sub shape {
    my $i = shift;
    my $kind = $i % 4;
    return Rect->new($i % 17 + 1, $i % 23 + 1) if $kind == 0;
    return Square->new($i % 31 + 1) if $kind == 1;
    return Triangle->new($i % 11 + 3, $i % 13 + 4, $i % 7 + 5) if $kind == 2;
    return Circle->new($i % 19 + 1);
}
sub score { my $total = 0; $total += $_->area * $_->weight + $_->perimeter for @{ $_[0] }; $total }
my @shapes = map { shape($_) } 0 .. 299999;
my $total = 0;
$total += score(\@shapes) for 1 .. 3;
print scalar(@shapes), " $total\n";
