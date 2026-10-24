#!/usr/bin/env python3
"""Бенчмарки движков Okkama: JVM, Sparrow (с JIT и без) и IR-интерпретатор.

Каждый каталог `NN-name/` — программа (`main.ok` и её модули). Раннер
собирает `.spb` и `.jar` (время и размер), затем исполняет каждую
конфигурацию `--runs` раз под `/usr/bin/time -l` (macOS): wall, user, sys,
пиковая память, инструкции и циклы CPU. Вывод всех конфигураций обязан
совпадать. Результат — `results/<время>.json` и `results/latest.json`.

    benchmarks/run.py [--runs 5] [--only 03-float,10-app] [--engines jvm,sparrow,sparrow-interp,ir]

Нужны собранные `okkama-rs/target/release/{okkama,sparrow}` и `java`.
Используется только стандартная библиотека Python.
"""

import argparse
import datetime
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
RELEASE = ROOT / "okkama-rs" / "target" / "release"
OKKAMA = RELEASE / "okkama"
SPARROW = RELEASE / "sparrow"
BUILD = HERE / "build"
RESULTS = HERE / "results"

TIME_FIELDS = {
    "maximum resident set size": "rss_bytes",
    "peak memory footprint": "footprint_bytes",
    "instructions retired": "instructions",
    "cycles elapsed": "cycles",
}


def measure(command, env=None):
    """Исполнить под `/usr/bin/time -l`: (stdout, метрики)."""
    # Режим JIT задаёт только конфигурация, а не окружение запуска раннера.
    full_env = {key: value for key, value in os.environ.items() if key not in ("OKKAMA_JIT", "OKKAMA_GC")}
    full_env.update(env or {})
    start = time.perf_counter()
    process = subprocess.run(["/usr/bin/time", "-l", *command], capture_output=True, env=full_env, cwd=ROOT)
    wall = time.perf_counter() - start
    stderr = process.stderr.decode(errors="replace")
    # Wall — часы Python (мкс); user/sys — `time` (шаг 10 мс).
    metrics = {"exit": process.returncode, "real": wall}
    head = re.search(r"([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys", stderr)
    if head:
        metrics.update(user=float(head.group(2)), sys=float(head.group(3)))
    for line in stderr.splitlines():
        match = re.match(r"\s*(\d+)\s+(.+?)\s*$", line)
        if match and match.group(2) in TIME_FIELDS:
            metrics[TIME_FIELDS[match.group(2)]] = int(match.group(1))
    program_stderr = "\n".join(
        line for line in stderr.splitlines()
        if not re.match(r"\s*[\d.]+ (real|user|sys)|\s*\d+\s+[a-z ]+$", line)
    )
    metrics["stderr"] = program_stderr.strip()
    return process.stdout, metrics


def timed(command):
    """Время команды сборки (wall) и успех."""
    start = datetime.datetime.now()
    process = subprocess.run(command, capture_output=True, cwd=ROOT)
    elapsed = (datetime.datetime.now() - start).total_seconds()
    if process.returncode != 0:
        sys.exit(f"build failed: {' '.join(map(str, command))}\n{process.stderr.decode()}")
    return elapsed


def sources(bench):
    files = sorted(bench.glob("*.ok"))
    return {
        "files": len(files),
        "lines": sum(len(path.read_text().splitlines()) for path in files),
        "bytes": sum(path.stat().st_size for path in files),
    }


def summarize(samples):
    keys = ["real", "user", "sys", "rss_bytes", "footprint_bytes", "instructions", "cycles"]
    summary = {}
    for key in keys:
        values = [sample[key] for sample in samples if key in sample]
        if values:
            summary[key] = statistics.median(values)
            summary[key + "_min"] = min(values)
    summary["cpu"] = summary.get("user", 0) + summary.get("sys", 0)
    return summary


def environment():
    def output(command):
        try:
            return subprocess.run(command, capture_output=True, text=True).stdout.strip() + subprocess.run(command, capture_output=True, text=True).stderr.strip()
        except OSError:
            return ""

    def sysctl(name):
        return output(["sysctl", "-n", name])

    java_home = output(["/usr/libexec/java_home"]) if Path("/usr/libexec/java_home").exists() else ""
    return {
        "date": datetime.datetime.now().isoformat(timespec="seconds"),
        "cpu": sysctl("machdep.cpu.brand_string"),
        "cores": sysctl("hw.ncpu"),
        "performance_cores": sysctl("hw.perflevel0.physicalcpu"),
        "memory_bytes": int(sysctl("hw.memsize") or 0),
        "os": f"{platform.system()} {platform.mac_ver()[0]}",
        "java": output(["java", "-version"]).splitlines()[0] if output(["java", "-version"]) else "",
        "java_home": java_home,
        "rustc": output(["rustc", "--version"]),
        "commit": output(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"]),
        "dirty": bool(output(["git", "-C", str(ROOT), "status", "--porcelain", "okkama-rs"])),
    }


def directory_size(path):
    return sum(file.stat().st_size for file in Path(path).rglob("*") if file.is_file())


def binaries(env):
    sizes = {
        "okkama": OKKAMA.stat().st_size,
        "sparrow": SPARROW.stat().st_size,
    }
    native = RELEASE / "libokkama_native.dylib"
    if native.exists():
        sizes["libokkama_native"] = native.stat().st_size
    if env.get("java_home"):
        sizes["jdk"] = directory_size(env["java_home"])
    return sizes


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--only", default="")
    parser.add_argument("--engines", default="jvm,sparrow,sparrow-interp,ir")
    args = parser.parse_args()
    for tool in (OKKAMA, SPARROW):
        if not tool.exists():
            sys.exit(f"missing {tool}: cargo build --release -p okkama-cli -p sparrow-cli")
    BUILD.mkdir(exist_ok=True)
    RESULTS.mkdir(exist_ok=True)
    only = {name for name in args.only.split(",") if name}
    benches = sorted(path for path in HERE.iterdir() if path.is_dir() and re.match(r"\d\d-", path.name))
    benches = [bench for bench in benches if not only or bench.name in only]
    engines = [engine for engine in args.engines.split(",") if engine]
    env = environment()
    report = {"environment": env, "binaries": binaries(env), "runs": args.runs, "benchmarks": []}
    for bench in benches:
        entry = bench / "main.ok"
        spb = BUILD / f"{bench.name}.spb"
        jar = BUILD / f"{bench.name}.jar"
        build = {
            "sparrow_seconds": timed([OKKAMA, "build", entry, "--target", "sparrow", "-o", spb]),
            "jvm_seconds": timed([OKKAMA, "build", entry, "--target", "jvm", "-o", jar]),
            "spb_bytes": spb.stat().st_size,
            "jar_bytes": jar.stat().st_size,
        }
        commands = {
            "jvm": (["java", "--enable-native-access=ALL-UNNAMED", "-jar", jar], {}),
            "sparrow": ([SPARROW, spb], {}),
            "sparrow-interp": ([SPARROW, spb], {"OKKAMA_JIT": "off"}),
            "ir": ([OKKAMA, "run", "--engine", "ir", entry], {}),
        }
        result = {"name": bench.name, "source": sources(bench), "build": build, "engines": {}}
        outputs = {}
        for engine in engines:
            command, extra = commands[engine]
            samples = []
            measure(command, extra)  # прогрев: кэш файлов и страниц бинарника
            for _ in range(args.runs):
                stdout, metrics = measure(command, extra)
                if metrics["exit"] != 0:
                    sys.exit(f"{bench.name} {engine} failed ({metrics['exit']}): {metrics['stderr'][:2000]}")
                outputs[engine] = hashlib.sha256(stdout).hexdigest()
                samples.append(metrics)
            result["engines"][engine] = summarize(samples)
            result["engines"][engine]["samples"] = [
                {key: value for key, value in sample.items() if key != "stderr"} for sample in samples
            ]
            print(f"{bench.name:16} {engine:15} {result['engines'][engine]['real']:7.3f}s "
                  f"cpu {result['engines'][engine]['cpu']:7.3f}s rss {result['engines'][engine].get('rss_bytes', 0) / 2**20:7.1f} MB",
                  flush=True)
        result["output_sha256"] = outputs
        result["outputs_agree"] = len(set(outputs.values())) == 1
        if not result["outputs_agree"]:
            print(f"!! {bench.name}: outputs differ between engines", flush=True)
        report["benchmarks"].append(result)
    stamp = datetime.datetime.now().strftime("%Y%m%dT%H%M%S")
    text = json.dumps(report, ensure_ascii=False, indent=2)
    # Частичный прогон (`--only`, `--engines`) не заменяет полный `latest.json`.
    full = not only and set(engines) == {"jvm", "sparrow", "sparrow-interp", "ir"}
    name = f"{stamp}.json" if full else f"partial-{stamp}.json"
    (RESULTS / name).write_text(text)
    if full:
        (RESULTS / "latest.json").write_text(text)
    print(f"results: {RESULTS / name}")


if __name__ == "__main__":
    main()
