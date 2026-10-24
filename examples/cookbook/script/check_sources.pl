# Проверка исходников apps и labs без запуска прикладных операций.
# Запуск на Perl 5.42: perl examples/cookbook/script/check_sources.pl
# Результат: source-checks.json с parser/semantic-проверкой каждой точки входа.
use v5.42;
use utf8;
use FindBin qw/$Bin/;
use Cwd qw/abs_path/;
use lib "$Bin/../../../okkama-pl/lib";
use Okkama::Lexer;use Okkama::Parser;use Okkama::Analyzer;
use Okkama::Semantic::ParseCache;use JSON::PP;use File::Find;
$|=1;
my $root=abs_path("$Bin/../../..");my $base="$root/examples/cookbook";
my @files;find(sub{push @files,$File::Find::name if /\.ok\z/ && -f $_},"$base/apps","$base/labs","$base/web");
my $cache=Okkama::Semantic::ParseCache->new(max_entries=>1024,max_source_bytes=>128*1024*1024);
my @results;
for my $path(sort @files){
 open my $file,'<:encoding(UTF-8)',$path or die $!;my $source=do{local $/;<$file>};close $file;
 my $directory=$path;$directory=~s{/[^/]+$}{};my $mode=$source=~/\btest\s+"/?'test':'production';
 my $ok=eval{my $ast=Okkama::Parser->new(tokens=>Okkama::Lexer::scan_tokens($source),source_name=>$path,mode=>$mode)->parse;
 Okkama::Analyzer->new(stdlib_dir=>"$root/std",import_base_dir=>$directory,parse_cache=>$cache,mode=>$mode)->analyze($ast);1;};
 my $name=substr($path,length($base)+1);push @results,{entry=>$name,passed=>$ok?JSON::PP::true:JSON::PP::false,($ok?():(error=>"$@"))};
 say "FAIL $name: $@" unless $ok;say 'Analyzed '.scalar(@results).'/'.scalar(@files) if @results%20==0;
}
my @bad=grep{!$_->{passed}}@results;
open my $out,'>:encoding(UTF-8)',"$base/source-checks.json" or die $!;print $out JSON::PP->new->canonical->pretty->encode(\@results);close $out;
say 'Source checks: '.(@results-@bad).'/'.scalar(@results);exit(@bad?1:0);
