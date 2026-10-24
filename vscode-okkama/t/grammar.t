use v5.42;
use utf8;

use FindBin;
use JSON::PP qw(decode_json);
use Test::More;

sub read_json($path) {
    open my $fh, '<:encoding(UTF-8)', $path
        or die "Cannot read $path: $!";
    local $/;
    return decode_json(<$fh>);
}

my $root = "$FindBin::Bin/..";
my $package = read_json("$root/package.json");
my $configuration = read_json("$root/language-configuration.json");
my $grammar = read_json("$root/syntaxes/okkama.tmLanguage.json");

is($package->{name}, 'okkama-vscode',
    'package keeps the installed extension identity');
is($package->{contributes}{languages}[0]{id}, 'okkama',
    'package contributes the Okkama language');
is($grammar->{scopeName}, 'source.okkama',
    'TextMate grammar uses the contributed source scope');
is_deeply($configuration->{comments}{blockComment}, ['/*', '*/'],
    'editor configuration declares Okkama block comments');

my $repository = $grammar->{repository};
for my $name (
    qw(
        rawStrings comments directives strings numbers declarations
        namedConstructorProjections keywords types functions
        invalidSyntax operators punctuation
    )
) {
    ok(exists $repository->{$name}, "grammar contains $name rules");
}

sub compile_patterns($value, $path = 'grammar') {
    if (ref $value eq 'HASH') {
        for my $key (sort keys $value->%*) {
            my $child = $value->{$key};
            if (($key eq 'match' || $key eq 'begin' || $key eq 'end')
                && !ref $child) {
                my $compiled = eval { qr/$child/ };
                ok(defined $compiled, "$path.$key compiles as a regular expression")
                    or diag($@);
            }
            else {
                compile_patterns($child, "$path.$key");
            }
        }
    }
    elsif (ref $value eq 'ARRAY') {
        for my $index (0 .. $value->$#*) {
            compile_patterns($value->[$index], "$path\[$index\]");
        }
    }
}

compile_patterns($grammar);

sub section_matches($section, $source) {
    for my $pattern ($repository->{$section}{patterns}->@*) {
        my $expression = $pattern->{match} // $pattern->{begin};
        next unless defined $expression;
        my $regex = qr/$expression/;
        return 1 if $source =~ $regex;
    }
    return 0;
}

for my $keyword (
    qw(
        module import take as native object role does enum has is private return
        test fake destroy let async await spawn if else match case for loop
        next last try catch confirm throw panic defer self true false
    )
) {
    ok(section_matches('keywords', $keyword),
        qq{current keyword "$keyword" is highlighted});
}

ok(!section_matches('keywords', 'allow'), 'removed grant word is an ordinary identifier');

for my $type (qw(int float rat decimal string char bool byte number any void NativeFailure)) {
    ok(section_matches('types', $type),
        qq{builtin type "$type" is highlighted});
}

for my $operator (
    qw(... .. ..= -> <- && || ! == != < <= > >= + - * / % = ^ & | ~ << >> >>>)
) {
    ok(section_matches('operators', $operator),
        qq{current operator "$operator" is highlighted});
}

ok(!section_matches('operators', 'xor'), 'xor remains an ordinary identifier');
ok(!section_matches('keywords', 'view Presentation'), 'removed view is not a keyword');
ok(!section_matches('declarations', 'view Presentation('), 'removed view declaration has no highlighting');
ok(section_matches('keywords', 'cap BusinessCard {'), 'contextual cap keyword');
ok(section_matches('declarations', 'cap BusinessCard {'), 'cap declaration');
ok(section_matches('declarations', 'cap Customer::BusinessCard {'), 'external cap declaration');
ok(section_matches('keywords', 'cap Customer::BusinessCard {'), 'external contextual cap keyword');
ok(section_matches('capAccess', 'Customer::first_name'), 'qualified owner field');
ok(section_matches('declarations', 'cap lowercase {'), 'cap names do not require case-based dispatch');
ok(section_matches('capAccess', '::BusinessCard'), 'cap selection');
ok(section_matches('capAccess', '::full_name'), 'computed field selection');
ok(section_matches('capAccess', '::UPPER'), 'uppercase computed field');
ok(section_matches('punctuation', '::'), 'cap separator');
for my $call ('test.skip("reason")','test.fail("message")',
    'fake.reading.get("key")','fake.writing.remove("key")') {
    ok(exists($repository->{contextualCapabilities}) && section_matches('contextualCapabilities',$call),
        "$call has contextual capability highlighting");
}
my @includes=map{$_->{include}//''}$grammar->{patterns}->@*;
my ($capability_index)=grep{$includes[$_] eq '#contextualCapabilities'}0..$#includes;
my ($keyword_index)=grep{$includes[$_] eq '#keywords'}0..$#includes;
ok(defined($capability_index) && $capability_index<$keyword_index,
    'contextual call scopes take priority over the test/fake keyword token');

for my $reserved (qw(isa super abstract protected null none nil with finally)) {
    ok(section_matches('invalidSyntax', $reserved),
        qq{reserved word "$reserved" is marked invalid});
}

ok(section_matches('directives', '#projected'),
    'default projection directive is highlighted');
ok(section_matches('directives', '#projected(of)'),
    'aliased projection directive is highlighted');
ok(section_matches('namedConstructorProjections', 'of Max(1, 2)'),
    'named constructor projection use is highlighted');
ok(!section_matches('invalidSyntax', '&'),
    'binary ampersand is valid; the parser rejects prefix projections');
ok(!section_matches('invalidSyntax', '~'), 'bitwise inversion is valid');
my ($bitwise_rule) = grep { $_->{name} eq 'keyword.operator.bitwise.okkama' }
    $repository->{operators}{patterns}->@*;
my $bitwise_regex = qr/$bitwise_rule->{match}/;
ok('&&' !~ $bitwise_regex && '||' !~ $bitwise_regex,
    'bitwise highlighting does not consume logical operators');
for my $literal ('0b1010_0011', '0X7FFF_FFFF_FFFF_FFFF', '1_000', '1_000.25_5') {
    my $matched = 0;
    for my $pattern ($repository->{numbers}{patterns}->@*) {
        my $regex = qr/$pattern->{match}/;
        $matched = 1 if $literal =~ /\A(?:$regex)\z/;
    }
    ok($matched, "$literal is highlighted as a whole number");
}
ok(section_matches('invalidSyntax', '?.'),
    'reserved optional access operator is marked invalid');
ok(section_matches('invalidSyntax', '**'),
    'reserved power operator is marked invalid');

my $raw_string = $repository->{rawStrings}{patterns}[0];
is($raw_string->{begin}, '```',
    'fenced raw string begins with exactly three backticks');
is($raw_string->{end}, '```',
    'fenced raw string ends with exactly three backticks');
is($raw_string->{name}, 'string.quoted.other.raw.fenced.okkama',
    'fenced raw string receives a raw string scope');
ok(section_matches('invalidSyntax', '`'),
    'an isolated backtick remains invalid outside a raw string');
ok(
    grep(
        $_->{open} eq '```' && $_->{close} eq '```',
        $configuration->{autoClosingPairs}->@*,
    ),
    'editor configuration auto-closes fenced raw strings',
);
ok(
    grep(
        $_->[0] eq '```' && $_->[1] eq '```',
        $configuration->{surroundingPairs}->@*,
    ),
    'editor configuration can surround text with a raw string fence',
);

for my $ordinary_identifier (qw(while in constructor adjust view mask cap)) {
    ok(!section_matches('keywords', $ordinary_identifier),
        qq{bare word "$ordinary_identifier" is not highlighted as a keyword});
}

ok(!section_matches('declarations', 'mask Card {'), 'former mask declaration has no cap highlighting');

done_testing;
