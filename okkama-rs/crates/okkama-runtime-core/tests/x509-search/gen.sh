#!/bin/sh
# Сертификаты построителя пути X.509 (S02, #17). EC P-256, срок 100 лет;
# проверки идут на явное время 2030-01-01. Перегенерация меняет DER, поэтому
# после неё обновляется и hex в conformance-кейсе L12.x509.path-search-budget.
#
#   root.der             якорь «Okkama Search Root»
#   leaf.der             лист search.invalid, издатель «Okkama Search Peer» (ключ K)
#   peer-00..11.der      самовыпущенные «Okkama Search Peer» с одним ключом K:
#                        подписывают друг друга, каждый с каждым
#   dead-end.der         «Okkama Search Peer» от корня без keyCertSign: якорь
#                        достижим, но любой путь через него неверен
#   ca-leaf.der          лист search.invalid, издатель «Okkama Search CA»
#   ca-forged-00..07.der «Okkama Search CA» с именем корня в издателе, но
#                        подписанные чужим ключом (без AKI)
#   ca.der               «Okkama Search CA» от корня — единственный верный путь
set -eu
cd "$(dirname "$0")"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
key() { openssl ecparam -genkey -name prime256v1 -noout -out "$work/$1.key"; }
ext() { printf '%s\n' "$@" > "$work/ext"; }
der() { openssl x509 -in "$work/$1.pem" -outform der -out "$1.der"; }
# sign NAME SUBJECT KEY ISSUER-PEM ISSUER-KEY
sign() {
    openssl req -new -key "$work/$3.key" -subj "/CN=$2" -out "$work/$1.csr"
    openssl x509 -req -in "$work/$1.csr" -CA "$work/$4.pem" -CAkey "$work/$5.key" -set_serial "0x$(openssl rand -hex 8)" \
        -days 36500 -sha256 -extfile "$work/ext" -out "$work/$1.pem"
    der "$1"
}
# self NAME SUBJECT KEY
self() {
    openssl req -new -x509 -key "$work/$3.key" -subj "/CN=$2" -set_serial "0x$(openssl rand -hex 8)" \
        -days 36500 -sha256 -config "$work/cnf" -extensions v3 -out "$work/$1.pem"
    der "$1"
}
ca_ext='basicConstraints=critical,CA:TRUE
keyUsage=critical,keyCertSign,cRLSign
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid'
printf '[req]\ndistinguished_name=dn\n[dn]\n[v3]\n%s:always\n' "$ca_ext" > "$work/cnf"

key root; key peer; key ca; key forger; key leaf
self root "Okkama Search Root" root
for i in 00 01 02 03 04 05 06 07 08 09 10 11; do
    self "peer-$i" "Okkama Search Peer" peer
done
leaf_ext() {
    ext 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature' 'extendedKeyUsage=serverAuth' \
        'subjectAltName=DNS:search.invalid' 'subjectKeyIdentifier=hash' 'authorityKeyIdentifier=keyid'
}
leaf_ext
sign leaf "search.invalid" leaf peer-00 peer
ext 'basicConstraints=critical,CA:TRUE' 'keyUsage=critical,digitalSignature' 'subjectKeyIdentifier=hash' 'authorityKeyIdentifier=keyid'
sign dead-end "Okkama Search Peer" peer root root
ext 'basicConstraints=critical,CA:TRUE' 'keyUsage=critical,keyCertSign,cRLSign' 'subjectKeyIdentifier=hash'
openssl req -new -x509 -key "$work/forger.key" -subj "/CN=Okkama Search Root" -days 36500 -sha256 \
    -config "$work/cnf" -extensions v3 -out "$work/forged-root.pem"
for i in 00 01 02 03 04 05 06 07; do
    sign "ca-forged-$i" "Okkama Search CA" ca forged-root forger
done
printf '%s\n' "$ca_ext" > "$work/ext"
sign ca "Okkama Search CA" ca root root
leaf_ext
sign ca-leaf "search.invalid" leaf ca ca
