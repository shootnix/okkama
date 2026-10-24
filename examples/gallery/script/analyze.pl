# Галерея: parser/semantic-проверка каждого исходника без выполнения main.
# Запуск через run check --analysis-only; stdout — JSON, включая ожидаемые отказы.
use v5.42;use utf8;use FindBin qw/$Bin/;use Cwd qw/abs_path/;
use lib "$Bin/../../../okkama-pl/lib";
use Okkama::Lexer;use Okkama::Parser;use Okkama::Analyzer;use Okkama::Semantic::ParseCache;
use JSON::PP;use File::Find;use Digest::SHA qw/sha256_hex/;use Encode qw/encode/;
my $base=abs_path("$Bin/..");my $root=abs_path("$base/../..");
open my $catalog_file,'<:encoding(UTF-8)',"$base/catalog.json" or die $!;
my $catalog=decode_json(encode('UTF-8',do{local $/;<$catalog_file>}));
my %expected=map{$_->{entry}=>$_->{error}}grep{$_->{mode} eq 'reject'}$catalog->{examples}->@*;
my @files;find(sub{push @files,$File::Find::name if /\.ok\z/ && -f $_},$base);
my $cache=Okkama::Semantic::ParseCache->new(max_entries=>1024,max_source_bytes=>128*1024*1024);my @results;
for my $path(sort @files){
 open my $f,'<:encoding(UTF-8)',$path or die $!;my $source=do{local $/;<$f>};close $f;
 my $source_hash=sha256_hex(encode('UTF-8',$source));
 my $directory=$path;$directory=~s{/[^/]+$}{};my $entry=substr($path,length($base)+1);
 my $ok=eval{my $ast=Okkama::Parser->new(tokens=>Okkama::Lexer::scan_tokens($source),source_name=>$path)->parse;
 Okkama::Analyzer->new(stdlib_dir=>"$root/std",import_base_dir=>$directory,parse_cache=>$cache)->analyze($ast);1;};my $error="$@";
 my $passed=exists($expected{$entry})?(!$ok && index($error,$expected{$entry})>=0):!!$ok;
 push @results,{entry=>$entry,passed=>$passed?JSON::PP::true:JSON::PP::false,expected_rejection=>exists($expected{$entry})?JSON::PP::true:JSON::PP::false,
 sha256=>$source_hash,($error?(diagnostic=>$error):())};
}
print JSON::PP->new->canonical->ascii->encode(\@results);exit((grep{!$_->{passed}}@results)?1:0);
