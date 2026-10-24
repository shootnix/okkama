#!/bin/sh
# Пересобирает рантайм JVM backend (okkama.rt) из Java-исходников.
#
# Готовые class-файлы хранятся в репозитории и встраиваются в компилятор
# (include_bytes!), поэтому сборка okkama-rs не требует JDK. Запускайте этот
# скрипт после изменения runtime/jvm/src и коммитьте обновлённые classes.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
runtime="$root/crates/okkama-compiler/runtime/jvm"
rm -rf "$runtime/classes"
mkdir -p "$runtime/classes"
javac --release 8 -Xlint:-options -encoding UTF-8 -d "$runtime/classes" \
    "$runtime"/src/okkama/rt/*.java
# Natives на Java FFM (SQLite) требуют JDK 22+: отдельный class-файл версии
# 66, загружается только программой, которая их вызывает.
javac --release 22 -Xlint:-options -encoding UTF-8 -cp "$runtime/classes" -d "$runtime/classes" \
    "$runtime"/src22/okkama/rt/*.java
# Данные рантайма (таблицы Unicode и т.п.) — ресурсы рядом с классами.
cp "$runtime"/src/okkama/rt/*.dat "$runtime/classes/okkama/rt/"
javac -version > "$runtime/classes/JAVAC_VERSION" 2>&1
echo "built $(ls "$runtime/classes/okkama/rt" | tr '\n' ' ')"
