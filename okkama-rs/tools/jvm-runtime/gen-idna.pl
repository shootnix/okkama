#!/usr/bin/env perl
# Таблицы UTS #46 для natives runtime/idna рантайма JVM из того же
# Net::IDN::UTS46 (2.591), что использует Perl-provider, и свойства Unicode
# (Bidi_Class, Joining_Type) из Unicode::UCD Perl. Вывод — gzip текста:
#   S <status> <from> <to>       статус: X disallowed, V3 disallowed_STD3_valid,
#                                M3 disallowed_STD3_mapped, I ignored,
#                                M mapped, D deviation, V valid
#   P <cp> <hex hex …>           отображение M/M3/D (пусто — удаление)
#   B <class> <from> <to>        Bidi_Class (короткое имя)
#   J <type> <from> <to>         Joining_Type (L, D, T, R, C, U)
use v5.36;
use Net::IDN::UTS46 ();
use Unicode::UCD qw/prop_invmap/;
use IO::Compress::Gzip qw/gzip $GzipError/;
no warnings 'utf8';

my $out = '';
my ($status, $from, $previous);
sub flush_run() {
    $out .= sprintf("S %s %X %X\n", $status, $from, $previous) if defined $status;
}
for my $cp (0 .. 0x10FFFF) {
    my $char = chr($cp);
    my ($kind, $map);
    if ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsDisallowed}/) { $kind = 'X' }
    elsif ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsDisallowedSTD3Valid}/) { $kind = 'V3' }
    elsif ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsDisallowedSTD3Mapped}/) { $kind = 'M3'; $map = Net::IDN::UTS46::_Mapping::MapDisallowedSTD3Mapped($char) }
    elsif ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsIgnored}/) { $kind = 'I' }
    elsif ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsMapped}/) { $kind = 'M'; $map = Net::IDN::UTS46::_Mapping::MapMapped($char) }
    elsif ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsDeviation}/) { $kind = 'D'; $map = Net::IDN::UTS46::_Mapping::MapDeviation($char) }
    elsif ($char =~ /\p{Net::IDN::UTS46::_Mapping::IsValid}/) { $kind = 'V' }
    else { $kind = 'X' }
    if (defined $map) {
        $out .= sprintf("P %X%s\n", $cp, join('', map { sprintf(' %X', ord) } split //, $map));
    }
    if (!defined($status) || $kind ne $status || $cp != $previous + 1) {
        flush_run();
        ($status, $from) = ($kind, $cp);
    }
    $previous = $cp;
}
flush_run();
for my $spec (['B', 'Bidi_Class'], ['J', 'Joining_Type']) {
    my ($tag, $property) = @$spec;
    my ($list, $map) = (prop_invmap($property))[0, 1];
    for my $i (0 .. $#$list) {
        my $to = $i < $#$list ? $list->[$i + 1] - 1 : 0x10FFFF;
        my $value = $map->[$i];
        $value = (Unicode::UCD::prop_value_aliases($property, $value))[0] // $value;
        $out .= sprintf("%s %s %X %X\n", $tag, $value, $list->[$i], $to);
    }
}
my $file = shift // die "usage: gen-idna.pl OUTPUT\n";
gzip(\$out => $file) or die "gzip failed: $GzipError\n";
