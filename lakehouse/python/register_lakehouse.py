#!/usr/bin/env python3
"""Register the QueryGraph lakehouse tables in a fresh Sail Spark session.

Sail's local catalog is session-scoped in the current setup. The data itself is
stable on disk under spark-warehouse. This script reads the QueryGraph manifest,
finds the matching Parquet directories, creates temporary views for the logical
qg_lakehouse tables, and verifies row counts.
"""
from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path

from pyspark.sql import SparkSession


@dataclass(frozen=True)
class TableSpec:
    logical_name: str
    bare_name: str
    rows: int
    location: Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--remote", default="sc://127.0.0.1:50051")
    parser.add_argument("--manifest", default=".querygraph/lakehouse/manifest/load-report.json")
    parser.add_argument("--warehouse", default="spark-warehouse")
    parser.add_argument("--create-global-temp", action="store_true")
    args = parser.parse_args()

    manifest = Path(args.manifest).resolve()
    warehouse = Path(args.warehouse).resolve()
    specs = load_specs(manifest, warehouse)

    spark = SparkSession.builder.remote(args.remote).getOrCreate()
    failures: list[str] = []
    for spec in specs:
        df = spark.read.parquet(str(spec.location))
        df.createOrReplaceTempView(spec.bare_name)
        if args.create_global_temp:
            df.createOrReplaceGlobalTempView(spec.bare_name)
        observed = df.count()
        ok = observed == spec.rows
        print(f"{spec.bare_name}\t{observed}\t{spec.location}\t{'ok' if ok else 'mismatch'}")
        if not ok:
            failures.append(f"{spec.bare_name}: expected {spec.rows}, observed {observed}")

    print(f"registered_tables={len(specs)}")
    if failures:
        raise SystemExit("\n".join(failures))


def load_specs(manifest: Path, warehouse: Path) -> list[TableSpec]:
    report = json.loads(manifest.read_text())
    specs: list[TableSpec] = []
    for dataset in report["datasets"]:
        for file in dataset["files"]:
            table = file.get("table")
            rows = file.get("rows")
            if not table or rows is None:
                continue
            bare = table.split(".", 1)[-1]
            location = find_matching_location(warehouse, bare, int(rows))
            specs.append(TableSpec(table, bare, int(rows), location))
    return specs


def find_matching_location(warehouse: Path, bare_name: str, rows: int) -> Path:
    matches = sorted(
        [p for p in warehouse.iterdir() if p.is_dir() and p.name.startswith(f"{bare_name}-")],
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    if not matches:
        raise FileNotFoundError(f"no Parquet directory found for {bare_name} in {warehouse}")
    # Most recent matching path is the current load. Row verification catches stale picks.
    return matches[0]


if __name__ == "__main__":
    main()
