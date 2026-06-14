# QueryGraph Lakehouse Demonstration

This subproject defines the downloadable demonstration lakehouse used by
`cargo run -- lakehouse-load`.

The loader materializes three layers:

1. Raw files under `.querygraph/lakehouse/datasets/<dataset>/raw`.
2. Prepared tabular files under `.querygraph/lakehouse/datasets/<dataset>/prepared`.
3. Semantic sidecars under `.querygraph/lakehouse/datasets/<dataset>/semantic`.

For tabular CSV, TSV, Dataverse `.tab`, and first-sheet XLSX files, the loader
infers Rust-side column types from a sample, creates raw Sail CSV views, and
then materializes typed Parquet tables in the configured Sail schema. For
non-tabular assets such as TIFF and PDF, the loader downloads the file and
records it in `lakehouse_files` with path, size, hash, content type, and parse
status.

The default schema is `qg_lakehouse`.

```bash
sail spark server --port 50051

cargo run -- lakehouse-load \
  --root .querygraph/lakehouse \
  --schema qg_lakehouse \
  --sail-endpoint http://127.0.0.1:50051
```

Verify the persisted typed tables against the manifest:

```bash
cargo run -- lakehouse-verify \
  --report .querygraph/lakehouse/manifest/load-report.json \
  --sail-endpoint http://127.0.0.1:50051
```

The loader uses Spark Connect session id `querygraph-lakehouse-loader`.
Use that same session id when inspecting the tables from a Spark Connect client.

The generated Sail tables include:

- one typed table per parseable tabular file;
- `qg_lakehouse.lakehouse_datasets`;
- `qg_lakehouse.lakehouse_files`;
- `qg_lakehouse.lakehouse_columns`.

Each dataset also gets:

- `semantic/croissant.json`;
- `semantic/cdif.json`.

The loader intentionally keeps full OpenLineage/DID audit integration in the
main QueryGraph e2e command. This subproject focuses on building an inspectable
Sail lakehouse corpus with typed data and semantic metadata sidecars.
