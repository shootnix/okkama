#!/usr/bin/env perl
# JSON benchmark in Perl (JSON::PP): parse, walk and write one document.
# usage: walk.pl <file>  — prints one JSON line with the phases in ms and the checksum.
use strict;
use warnings;
no warnings "recursion";
use JSON::PP;
use Time::HiRes qw(time);
use B;

my @sums = (0, 0, 0, 0);

sub walk {
    my ($value) = @_;
    $sums[0]++;
    my $type = ref $value;
    if ($type eq 'HASH') {
        for my $name (keys %$value) {
            $sums[1] += length $name;
            walk($value->{$name});
        }
    } elsif ($type eq 'ARRAY') {
        walk($_) for @$value;
    } elsif ($type eq 'JSON::PP::Boolean') {
        $sums[3]++ if $value;
    } elsif (defined $value) {
        my $flags = B::svref_2object(\$value)->FLAGS;
        if (($flags & (B::SVf_IOK | B::SVf_NOK)) && !($flags & B::SVf_POK)) { $sums[2]++ }
        else { $sums[1] += length $value }
    }
}

my $file = shift;
open my $fh, '<:raw', $file or die "$file: $!";
my $text = do { local $/; <$fh> };
my $json = JSON::PP->new->utf8;
my $start = time;
my $document = $json->decode($text);
my $parsed = time;
walk($document);
my $walked = time;
my $out = $json->encode($document);
my $written = time;
printf "{\"parse\":%.3f,\"walk\":%.3f,\"write\":%.3f,\"nodes\":%d,\"chars\":%d,\"numbers\":%d,\"trues\":%d,\"length\":%d}\n",
    ($parsed - $start) * 1000, ($walked - $parsed) * 1000, ($written - $walked) * 1000, @sums, length $out;
