use strict; use warnings;
# Коллекции: map/grep/суммы, сортировка, хэши и частоты.
use List::Util qw(sum0);
my @values = map { ($_ * 7919 + 13) % 10007 } 0 .. 199999;
my @evens = grep { $_ % 2 == 0 } @values;
my @squares = map { $_ * $_ % 1000003 } @evens;
my $sum = sum0(@squares);
my @sorted = sort { $a <=> $b } @values[0 .. 49999];
my %buckets;
$buckets{$_ % 97}++ for @values[0 .. 3999];
my %index = (low => scalar(grep { $_ < 5000 } @values), high => scalar(grep { $_ >= 5000 } @values));
print join(" ", $sum, $sorted[0], $sorted[49999], scalar(keys %buckets), $buckets{0} // 0, $index{low}, $index{high}), "\n";
