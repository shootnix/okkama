use strict; use warnings;
# Объекты: миллион шагов неизменяемых объектов (каждый шаг — новый объект).
package Particle;
sub new { my ($class, $x, $y, $vx, $vy, $bounces) = @_; bless { x => $x, y => $y, vx => $vx, vy => $vy, bounces => $bounces // 0 }, $class }
sub energy { my $s = shift; $s->{vx} * $s->{vx} + $s->{vy} * $s->{vy} }
sub position { my $s = shift; $s->{x} * 1000 + $s->{y} }
sub bounces { $_[0]{bounces} }
sub moved {
    my $s = shift;
    my $nx = $s->{x} + $s->{vx};
    my $ny = $s->{y} + $s->{vy};
    my $hit_x = $nx < 0 || $nx > 1000;
    my $hit_y = $ny < 0 || $ny > 1000;
    Particle->new(
        $hit_x ? $s->{x} : $nx,
        $hit_y ? $s->{y} : $ny,
        $hit_x ? -$s->{vx} : $s->{vx},
        $hit_y ? -$s->{vy} : $s->{vy},
        ($hit_x || $hit_y) ? $s->{bounces} + 1 : $s->{bounces},
    );
}
package main;
sub simulate {
    my ($id, $steps) = @_;
    my $p = Particle->new($id * 37 % 1000, $id * 91 % 1000, $id % 13 + 1, $id % 7 + 2);
    $p = $p->moved for 1 .. $steps;
    $p->position + $p->bounces * 7 + $p->energy;
}
my $total = 0;
$total += simulate($_, 2500) for 0 .. 399;
print "$total\n";
