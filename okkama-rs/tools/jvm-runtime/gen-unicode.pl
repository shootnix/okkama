#!/usr/bin/env perl
# Таблицы Unicode для natives runtime/unicode_character рантайма JVM из
# Unicode::UCD того Perl, на котором работает эталонный интерпретатор:
# так имена, категории, combining class, числовые значения и свойства
# ширины совпадают с Perl-provider. Вывод — gzip текстового файла:
#   R <prop> <from> <to>         диапазон с булевым свойством
#   G <cat> <from> <to>          общая категория
#   C <ccc> <from> <to>          combining class (кроме 0)
#   N <cp> <value>               числовое значение (как в charinfo)
#   M <cp> <name>                имя символа
#   A <prefix> <from> <to>       алгоритмические имена <prefix>-<HEX>
#   H <from> <to>                слоги хангыль (имя по алгоритму Unicode)
use v5.36;
use Unicode::UCD qw/charinfo charprop prop_invlist prop_invmap/;
use IO::Compress::Gzip qw/gzip $GzipError/;

my $out = '';
sub ranges($tag, $invlist) {
    my @list = @$invlist;
    for (my $i = 0; $i < @list; $i += 2) {
        my $from = $list[$i];
        my $to = $i + 1 < @list ? $list[$i + 1] - 1 : 0x10FFFF;
        $out .= sprintf("R %s %X %X\n", $tag, $from, $to);
    }
}
ranges('M', [prop_invlist('General_Category=Mark')]);
ranges('DI', [prop_invlist('Default_Ignorable_Code_Point')]);
ranges('RI', [prop_invlist('Regional_Indicator')]);
ranges('E', [prop_invlist('Emoji')]);
ranges('EP', [prop_invlist('Extended_Pictographic')]);
ranges('EPres', [prop_invlist('Emoji_Presentation')]);
ranges('W', [prop_invlist('East_Asian_Width=Wide')]);
ranges('F', [prop_invlist('East_Asian_Width=Fullwidth')]);

my ($list, $map) = (prop_invmap('General_Category'))[0, 1];
for my $i (0 .. $#$list) {
    my $to = $i < $#$list ? $list->[$i + 1] - 1 : 0x10FFFF;
    $out .= sprintf("G %s %X %X\n", $map->[$i], $list->[$i], $to);
}
($list, $map) = (prop_invmap('Canonical_Combining_Class'))[0, 1];
for my $i (0 .. $#$list) {
    my $to = $i < $#$list ? $list->[$i + 1] - 1 : 0x10FFFF;
    my $value = $map->[$i];
    # prop_invmap даёт короткие имена классов; число берём из charinfo.
    my $number = (charinfo($list->[$i]) // {})->{combining} // 0;
    next unless $number;
    $out .= sprintf("C %d %X %X\n", $number, $list->[$i], $to);
}
my @runs;
sub run($kind, $prefix, $cp) {
    if (@runs && $runs[-1][0] eq $kind && $runs[-1][1] eq $prefix && $runs[-1][3] == $cp - 1) {
        $runs[-1][3] = $cp;
    } else {
        push @runs, [$kind, $prefix, $cp, $cp];
    }
}
for my $cp (0 .. 0x10FFFF) {
    next if $cp >= 0xD800 && $cp <= 0xDFFF;
    my $info = charinfo($cp) // next;
    $out .= sprintf("N %X %s\n", $cp, $info->{numeric}) if $info->{numeric} ne '';
    my $name = $info->{name};
    next if !defined($name) || $name eq '' || $name =~ /\A<.*>\z/;
    if ($name =~ /\A(.+)-([0-9A-F]+)\z/ && hex($2) == $cp) {
        run('A', $1, $cp);
        next;
    }
    if ($name =~ /\AHANGUL SYLLABLE /) {
        run('H', '', $cp);
        next;
    }
    $out .= sprintf("M %X %s\n", $cp, $name);
}
for my $run (@runs) {
    if ($run->[0] eq 'A') {
        $out .= sprintf("A %s %X %X\n", $run->[1] =~ s/ /_/gr, $run->[2], $run->[3]);
    } else {
        $out .= sprintf("H %X %X\n", $run->[2], $run->[3]);
    }
}
my $file = shift // die "usage: gen-unicode.pl OUTPUT\n";
gzip(\$out => $file) or die "gzip failed: $GzipError\n";
