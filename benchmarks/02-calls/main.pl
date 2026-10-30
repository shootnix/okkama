use strict; use warnings;
# Вызовы: рекурсивный fib (~7 млн вызовов).
package Fib;
sub new { bless {}, shift }
sub fib { my ($self, $n) = @_; $n < 2 ? $n : $self->fib($n - 1) + $self->fib($n - 2) }
package main;
print Fib->new->fib(32), "\n";
