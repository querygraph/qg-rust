# QueryGraph releases

QueryGraph versions are `0.MINOR.PATCH` SemVer (in `0.x`, a minor may include
breaking changes). Starting with `0.2.0`, each release carries a **codename from
the birds-of-prey pool** (`~/src/names/BIRDS.md`), assigned in list order. The
release tracks specific upstream releases of its sibling crates — Grust,
TypeSec, and LakeCat — which carry their own codenames.

## Release log

| Version | Codename | Notes |
|---|---|---|
| 0.2.0 | Peregrine | First **named** QueryGraph release. Tracks Grust 0.11.0 "Crab", TypeSec 0.11.0 "Burano", and LakeCat 0.2.1 "Lynx". Adopts Crab's `grust-cypher` reads (catalog/semantic-graph `MATCH`/`CALL db.labels()`), surfaces Burano's audit-safe TypeDID attestations, migrates the catalog gate onto LakeCat's shared `qglake-bundle` crate (deleting the copied wire format), and splits the source into human-size (≤500-line) modules. |
| 0.1.1 | — | (pre-codename) Published Grust 0.10.0, TypeSec 0.8.0. |
| 0.1.0 | — | (pre-codename) Initial all-Rust AI Navigator semantic layer. |

## Codename pool (birds of prey, in assignment order)

Names already assigned are struck through.

1. ~~Peregrine~~ — assigned to `0.2.0`
2. Goshawk
3. Sentinel
4. Harrier
5. Merlin
6. Gyrfalcon
7. Talon
8. Falcon
9. Raptor
10. Accipiter
11. Kestrel
12. Strix
13. Aquila
14. Buteo
15. Tercel
16. Caracara
17. Shrike
18. Stoop
19. Eyrie
20. Verreaux
21. Eagle
22. Harpy
23. Imperial
24. Golden
25. Aerie
