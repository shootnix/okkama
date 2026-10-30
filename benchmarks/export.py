#!/usr/bin/env python3
"""Наборы данных отчёта о бенчмарках из `results/*.json` (`run.py`).

    benchmarks/export.py RESULTS.json [--before OLD.json] [--out DIR] [--title TEXT]

Пишет в DIR (по умолчанию `results/export`) CSV-файлы и отчёт:

- `samples.csv` — каждый замер: тест, движок, номер, wall, user, sys, cpu,
  RSS и footprint (МиБ), инструкции и циклы;
- `builds.csv` — время сборки `.spb`/`.jar` и их размеры, строки исходника;
- `installs.csv` — размеры бинарников и установок движков;
- `before_after.csv` — медианы Sparrow (с JIT и без) до и после, если дан
  `--before`;
- `environment.csv` — машина и версии;
- `stats.csv` — по каждой паре «тест × движок»: n, среднее, медиана,
  квартили, минимум, максимум, стандартное отклонение и CV wall-time,
  медианы CPU (user, sys), RSS, инструкций и IPC;
- `summary.csv` — по движку: среднее геометрическое отношений медиан к
  Python, Perl (без 04-bigint), Ruby и Okkama JVM по тестам 01–10, запуск,
  суммы CPU и медианы памяти;
- `REPORT.md` — те же данные таблицами.

Используется только стандартная библиотека Python.
"""

import argparse
import csv
import json
import math
import statistics
from pathlib import Path

HERE = Path(__file__).resolve().parent
MIB = 1024 * 1024

ORDER = ["sparrow", "sparrow-interp", "ir", "jvm", "python", "perl", "ruby"]

ENGINE_NAMES = {
    "sparrow": "Sparrow (JIT)",
    "sparrow-interp": "Sparrow (interpreter)",
    "ir": "Okkama IR",
    "jvm": "Okkama JVM",
    "python": "Python",
    "perl": "Perl",
    "ruby": "Ruby",
}


def write(path, columns, rows):
    with open(path, "w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        for row in rows:
            writer.writerow(row)


def quantile(values, q):
    ordered = sorted(values)
    position = (len(ordered) - 1) * q
    low, high = math.floor(position), math.ceil(position)
    return ordered[low] + (ordered[high] - ordered[low]) * (position - low)


def statistics_rows(report):
    """Статистика по каждой паре «тест × движок» из всех замеров."""
    rows = []
    for bench in report["benchmarks"]:
        for engine in [engine for engine in ORDER if engine in bench["engines"]]:
            samples = bench["engines"][engine]["samples"]
            wall = [sample["real"] for sample in samples]
            mean = statistics.fmean(wall)
            deviation = statistics.stdev(wall) if len(wall) > 1 else 0.0

            def median(key, scale=1.0):
                values = [sample[key] / scale for sample in samples if key in sample]
                return statistics.median(values) if values else float("nan")

            cpu = [sample.get("user", 0) + sample.get("sys", 0) for sample in samples]
            rss = [sample.get("rss_bytes", 0) / MIB for sample in samples]
            rows.append({
                "bench": bench["name"],
                "engine": ENGINE_NAMES[engine],
                "n": len(wall),
                "mean": round(mean, 4),
                "median": round(statistics.median(wall), 4),
                "q1": round(quantile(wall, 0.25), 4),
                "q3": round(quantile(wall, 0.75), 4),
                "min": round(min(wall), 4),
                "max": round(max(wall), 4),
                "sd": round(deviation, 4),
                "cv_pct": round(deviation / mean * 100, 1),
                "cpu_median": round(statistics.median(cpu), 3),
                "user_median": round(median("user"), 3),
                "sys_median": round(median("sys"), 3),
                "rss_median": round(statistics.median(rss), 1),
                "rss_min": round(min(rss), 1),
                "rss_max": round(max(rss), 1),
                "ginstructions": round(median("instructions", 1e9), 3),
                "ipc": round(median("instructions") / median("cycles"), 3),
            })
    return rows


def summary_rows(stats):
    """Средние геометрические отношений медиан по тестам 01–10."""
    at = {(row["bench"], row["engine"]): row for row in stats}
    benches = sorted({row["bench"] for row in stats if row["bench"] != "00-hello"})

    def ratio(engine, reference, skip=None):
        values = [
            at[(bench, reference)]["median"] / at[(bench, engine)]["median"]
            for bench in benches
            if bench != skip and (bench, reference) in at and (bench, engine) in at
        ]
        return round(statistics.geometric_mean(values), 2) if values else ""

    rows = []
    for engine in [ENGINE_NAMES[engine] for engine in ORDER]:
        own = [row for row in stats if row["engine"] == engine and row["bench"] != "00-hello"]
        if not own:
            continue
        hello = at.get(("00-hello", engine))
        rows.append({
            "engine": engine,
            "vs_python": ratio(engine, "Python"),
            "vs_perl": ratio(engine, "Perl", "04-bigint"),
            "vs_ruby": ratio(engine, "Ruby"),
            "vs_jvm": ratio(engine, "Okkama JVM"),
            "startup_ms": round(hello["median"] * 1000, 1) if hello else "",
            "wall_total": round(sum(row["median"] for row in own), 3),
            "cpu_total": round(sum(row["cpu_median"] for row in own), 2),
            "user_total": round(sum(row["user_median"] for row in own), 2),
            "sys_total": round(sum(row["sys_median"] for row in own), 2),
            "rss_median": round(statistics.median(row["rss_median"] for row in own), 1),
            "rss_max": max(row["rss_max"] for row in own),
            "ginstructions_total": round(sum(row["ginstructions"] for row in own), 2),
            "ipc": round(statistics.median(row["ipc"] for row in own), 2),
        })
    return rows


def table(header, rows):
    lines = ["| " + " | ".join(header) + " |", "|" + "|".join("---" if i == 0 else "---:" for i in range(len(header))) + "|"]
    lines += ["| " + " | ".join(str(cell) for cell in row) + " |" for row in rows]
    return "\n".join(lines)


def markdown(title, report, stats, summary, installs, builds, before_after):
    env = report["environment"]
    engines = [ENGINE_NAMES[engine] for engine in ORDER if any(row["engine"] == ENGINE_NAMES[engine] for row in stats)]
    benches = sorted({row["bench"] for row in stats})
    at = {(row["bench"], row["engine"]): row for row in stats}
    seconds = lambda value: f"{value:.3f}"
    cell = lambda bench, engine, key, fmt: fmt(at[(bench, engine)][key]) if (bench, engine) in at else "—"

    def spread(bench, engine):
        row = at.get((bench, engine))
        return f"{row['mean']:.3f} ({row['min']:.3f}…{row['max']:.3f})" if row else "—"
    parts = [
        f"# {title}",
        "",
        "Сгенерировано `benchmarks/export.py` из `results.json`; не правится руками.",
        "",
        f"{env.get('cpu')}, {env.get('memory_bytes', 0) // 1024 ** 3} GiB, {env.get('os')}; "
        f"{report['runs']} замеров на пару после прогрева; {env.get('date')}, коммит `{env.get('commit')}`"
        + (" (с незакоммиченными изменениями)" if env.get("dirty") else "") + ".",
        f"Версии: {env.get('python')}, Perl {env.get('perl')}, {env.get('ruby')}, {env.get('java')}, {env.get('rustc')}.",
        "",
        "## Сводка",
        "",
        "Среднее геометрическое отношений медианного wall-time по тестам 01–10 (больше 1 — движок быстрее).",
        "Для Perl тест 04-bigint исключён: `Math::BigInt` без GMP.",
        "",
        table(["Движок", "vs Python", "vs Perl", "vs Ruby", "vs Okkama JVM", "Запуск, мс", "CPU, с", "RSS медиана, МиБ", "RSS макс, МиБ", "IPC"],
              [[row["engine"], row["vs_python"], row["vs_perl"], row["vs_ruby"], row["vs_jvm"], row["startup_ms"], row["cpu_total"],
                row["rss_median"], row["rss_max"], row["ipc"]] for row in summary]),
        "",
        "## Медиана wall-time, с",
        "",
        table(["Тест", *engines], [[bench, *[cell(bench, engine, "median", seconds) for engine in engines]] for bench in benches]),
        "",
        "## Во сколько раз быстрее Python",
        "",
        table(["Тест", *[engine for engine in engines if engine != "Python"]],
              [[bench, *[f"{at[(bench, 'Python')]['median'] / at[(bench, engine)]['median']:.2f}" if (bench, engine) in at and (bench, "Python") in at else "—"
                          for engine in engines if engine != "Python"]] for bench in benches]),
        "",
        "## Среднее, минимум и максимум wall-time, с",
        "",
        table(["Тест", *engines],
              [[bench, *[spread(bench, engine) for engine in engines]] for bench in benches]),
        "",
        "## Коэффициент вариации wall-time, %",
        "",
        table(["Тест", *engines], [[bench, *[cell(bench, engine, "cv_pct", str) for engine in engines]] for bench in benches]),
        "",
        "## Процессорное время (user + sys), медиана, с",
        "",
        table(["Тест", *engines], [[bench, *[cell(bench, engine, "cpu_median", lambda v: f"{v:.2f}") for engine in engines]] for bench in benches]),
        "",
        "## Пиковый RSS, медиана, МиБ",
        "",
        table(["Тест", *engines], [[bench, *[cell(bench, engine, "rss_median", lambda v: f"{v:.1f}") for engine in engines]] for bench in benches]),
        "",
        "## Инструкции процессора, медиана, млрд",
        "",
        table(["Тест", *engines], [[bench, *[cell(bench, engine, "ginstructions", lambda v: f"{v:.3f}") for engine in engines]] for bench in benches]),
        "",
        "## Размеры",
        "",
        table(["Что", "Вид", "МиБ"], [[row["engine"], row["part"], row["mib"]] for row in installs]),
        "",
        "## Сборка",
        "",
        table(["Тест", "Файлов", "Строк", "Сборка .spb, с", ".spb, КиБ", "Сборка JAR, с", "JAR, КиБ"],
              [[row["bench"], row["source_files"], row["source_lines"], row["spb_build_s"], row["spb_kib"], row["jar_build_s"], row["jar_kib"]] for row in builds]),
    ]
    if before_after:
        parts += [
            "",
            "## Sparrow до и после",
            "",
            table(["Тест", "Движок", "До, с", "После, с", "Ускорение"],
                  [[row["bench"], row["engine"], row["before_s"], row["after_s"], f"{row['speedup']:.2f}×"] for row in before_after]),
        ]
    return "\n".join(parts) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("results")
    parser.add_argument("--before")
    parser.add_argument("--out", default=str(HERE / "results" / "export"))
    parser.add_argument("--title", default="Бенчмарки Okkama")
    args = parser.parse_args()
    report = json.loads(Path(args.results).read_text())
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    samples = []
    builds = []
    for bench in report["benchmarks"]:
        for engine, summary in bench["engines"].items():
            for run, sample in enumerate(summary["samples"], 1):
                samples.append({
                    "bench": bench["name"],
                    "engine": ENGINE_NAMES.get(engine, engine),
                    "run": run,
                    "wall_s": round(sample["real"], 6),
                    "user_s": sample.get("user", ""),
                    "sys_s": sample.get("sys", ""),
                    "cpu_s": round(sample.get("user", 0) + sample.get("sys", 0), 3),
                    "rss_mib": round(sample.get("rss_bytes", 0) / MIB, 2),
                    "footprint_mib": round(sample.get("footprint_bytes", 0) / MIB, 2),
                    "instructions": sample.get("instructions", ""),
                    "cycles": sample.get("cycles", ""),
                })
        builds.append({
            "bench": bench["name"],
            "source_lines": bench["source"]["lines"],
            "source_files": bench["source"]["files"],
            "spb_build_s": round(bench["build"]["sparrow_seconds"], 4),
            "jar_build_s": round(bench["build"]["jvm_seconds"], 4),
            "spb_kib": round(bench["build"]["spb_bytes"] / 1024, 2),
            "jar_kib": round(bench["build"]["jar_bytes"] / 1024, 2),
            "outputs_agree": bench["outputs_agree"],
        })
    write(out / "samples.csv", list(samples[0]), samples)
    write(out / "builds.csv", list(builds[0]), builds)

    sizes = report["binaries"]
    installs = [
        {"engine": "Okkama (okkama + sparrow + native lib + std)", "part": "install",
         "mib": round(sizes["okkama_install"] / MIB, 2)},
        {"engine": "okkama (compiler, IR, LSP)", "part": "binary", "mib": round(sizes["okkama"] / MIB, 2)},
        {"engine": "sparrow (VM with JIT)", "part": "binary", "mib": round(sizes["sparrow"] / MIB, 2)},
    ]
    if "libokkama_native" in sizes:
        installs.append({"engine": "libokkama_native (JVM natives)", "part": "binary",
                         "mib": round(sizes["libokkama_native"] / MIB, 2)})
    if "jdk" in sizes:
        installs.append({"engine": "JDK (Okkama JVM)", "part": "install", "mib": round(sizes["jdk"] / MIB, 2)})
    for engine in ("python", "perl", "ruby"):
        if engine + "_install" in sizes:
            installs.append({"engine": ENGINE_NAMES[engine], "part": "install",
                             "mib": round(sizes[engine + "_install"] / MIB, 2)})
    write(out / "installs.csv", ["engine", "part", "mib"], installs)

    if args.before:
        before = json.loads(Path(args.before).read_text())
        old = {bench["name"]: bench["engines"] for bench in before["benchmarks"]}
        rows = []
        for bench in report["benchmarks"]:
            for engine in ("sparrow", "sparrow-interp"):
                if engine in bench["engines"] and engine in old.get(bench["name"], {}):
                    was = old[bench["name"]][engine]["real"]
                    now = bench["engines"][engine]["real"]
                    rows.append({"bench": bench["name"], "engine": ENGINE_NAMES[engine],
                                 "before_s": round(was, 4), "after_s": round(now, 4),
                                 "speedup": round(was / now, 3)})
        write(out / "before_after.csv", ["bench", "engine", "before_s", "after_s", "speedup"], rows)
    else:
        rows = []
    before_after = rows

    env = report["environment"]
    keys = ["date", "cpu", "cores", "performance_cores", "os", "rustc", "java", "python", "perl", "ruby", "commit"]
    rows = [{"key": key, "value": env.get(key, "")} for key in keys]
    rows.append({"key": "memory", "value": f"{env.get('memory_bytes', 0) // (1024 ** 3)} GiB"})
    rows.append({"key": "runs", "value": report["runs"]})
    write(out / "environment.csv", ["key", "value"], rows)

    stats = statistics_rows(report)
    write(out / "stats.csv", list(stats[0]), stats)
    summary = summary_rows(stats)
    write(out / "summary.csv", list(summary[0]), summary)
    (out / "REPORT.md").write_text(markdown(args.title, report, stats, summary, installs, builds, before_after))
    print(f"written: {out}")


if __name__ == "__main__":
    main()
