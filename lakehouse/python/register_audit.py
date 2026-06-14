#!/usr/bin/env python3
"""Register QueryGraph OpenLineage audit tables in a fresh Sail Spark session."""
from __future__ import annotations

import argparse
from pathlib import Path

from pyspark.sql import SparkSession


AUDIT_TABLES = ("openlineage_events", "openlineage_attestations")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--remote", default="sc://127.0.0.1:50051")
    parser.add_argument("--warehouse", default="spark-warehouse")
    parser.add_argument("--create-global-temp", action="store_true")
    args = parser.parse_args()

    warehouse = Path(args.warehouse).resolve()
    spark = SparkSession.builder.remote(args.remote).getOrCreate()
    for table in AUDIT_TABLES:
        location = find_latest_location(warehouse, table)
        df = spark.read.parquet(str(location))
        df.createOrReplaceTempView(table)
        if args.create_global_temp:
            df.createOrReplaceGlobalTempView(table)
        rows = df.count()
        print(f"{table}\t{rows}\t{location}\tok")


def find_latest_location(warehouse: Path, table: str) -> Path:
    matches = sorted(
        [p for p in warehouse.iterdir() if p.is_dir() and p.name.startswith(f"{table}-")],
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    if not matches:
        raise FileNotFoundError(f"no Parquet directory found for {table} in {warehouse}")
    return matches[0]


if __name__ == "__main__":
    main()
