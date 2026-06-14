# QueryGraph Sail PySpark Client

This uv project provides a Python 3.14 / PySpark client environment for the
local Sail lakehouse.

## Setup

```bash
asdf install python 3.14.6
asdf set python 3.14.6
uv sync --project lakehouse/python --python /Users/alexy/.asdf/installs/python/3.14.6/bin/python
```

## Start Sail with the uv Python environment

```bash
PATH="$PWD/lakehouse/python/.venv/bin:$PATH" \
VIRTUAL_ENV="$PWD/lakehouse/python/.venv" \
sail spark server --port 50051
```

## Register the lakehouse in the running Sail server

```bash
uv run --project lakehouse/python python lakehouse/python/register_lakehouse.py \
  --remote sc://127.0.0.1:50051 \
  --manifest .querygraph/lakehouse/manifest/load-report.json \
  --warehouse spark-warehouse \
  --create-global-temp
```

The script verifies every table row count while registering it.

Register the OpenLineage audit tables emitted by `dataverse-e2e --live-sail`:

```bash
uv run --project lakehouse/python python lakehouse/python/register_audit.py \
  --remote sc://127.0.0.1:50051 \
  --warehouse spark-warehouse \
  --create-global-temp
```

## Open a PySpark shell

```bash
uv run --project lakehouse/python pyspark --remote sc://127.0.0.1:50051
```

Then query the globally registered views:

```python
spark.sql("SELECT COUNT(*) FROM global_temp.government_finance__countydata").show()
spark.sql("SELECT * FROM global_temp.codata_constants_2022__codata_constants_2022 LIMIT 5").show(truncate=False)
spark.sql("SELECT event_hash, event_type, job_name FROM global_temp.openlineage_events").show(truncate=False)
```

In this Sail setup, use `global_temp.<table>` from a fresh client session. Plain
unqualified temporary views are session-local.
