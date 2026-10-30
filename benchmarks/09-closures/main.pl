use strict; use warnings;
# Замыкания и функции высшего порядка: композиция, захват, функции в списках.
use List::Util qw(reduce);
sub compose { my ($f, $g) = @_; sub { $g->($f->($_[0])) } }
sub adder { my $amount = shift; sub { $_[0] + $amount } }
sub multiplier { my $factor = shift; sub { $_[0] * $factor % 1000003 } }
my @stages = map { $_ % 2 == 0 ? adder($_ + 1) : multiplier($_ + 2) } 0 .. 7;
my $combined = sub { $_[0] };
$combined = compose($combined, $_) for @stages;
my $total = 0;
$total = ($total + $combined->($_)) % 1000000007 for 0 .. 599999;
my @counters = map { adder($_) } 0 .. 49999;
my $applied = 0;
$applied = $_->($applied) % 1000003 for @counters;
print "$total $applied\n";
