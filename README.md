# KM003C resources

An independent archive of manufacturer firmware, Windows applications and manuals
for POWER-Z KM003C / KM002C, maintained alongside
[km003c-rs](https://github.com/okhsunrog/km003c-rs).

- [Browse firmware and resources](https://okhsunrog.github.io/km003c-resources/)
- [Firmware catalog](https://okhsunrog.github.io/km003c-resources/catalog.json)
- [Archive downloads](https://github.com/okhsunrog/km003c-resources/releases/tag/archive-2026-10-28)
- [Resource provenance and checksums](resources.json)

Only KM003C firmware packages are kept in Git and published on Pages. Windows
applications, manuals and complete source archives are Release assets. Installers
and applications were unpacked and indexed without executing them. The 2.5.1 /
2.5.2 RAR files use the manufacturer-provided password `chargerlab`.

The catalog includes 21 packages from the initial archive. Embedded version,
build date, CRC, package SHA-256 and release notes are checked with the same Rust
firmware parser used by km003c-rs. Package validation is not a claim that all
versions have been flashed or tested on every hardware revision.

Original names are retained as provenance, but identities and download paths
come from embedded metadata and the package hash. The archive's
`KM003C_V2.0.0.mencrypt` identifies itself as 2.0.1 (2026.4.9); another 2.0.1
build is dated 2026.4.21. Neither is silently relabeled or replaced.

## Add firmware

1. Record the manufacturer package's provenance in `resources.json`. If adding
   an archive asset, upload it to a new Release and record its URL, size and
   SHA-256. Never replace a published package with different bytes.
2. From this repository, import using its collection ID:

   ```sh
   cargo run --locked -- import /path/to/KM003C_Vx.y.z.mencrypt --source COLLECTION_ID
   ```

   Multiple packages can be passed at once. Duplicate hashes leave existing
   packages unchanged. Original bytes are preserved, including outer trailers.
3. Review the registry and generated catalog:

   ```sh
   cargo run --locked -- build --check
   cargo fmt --check
   cargo clippy --locked --all-targets -- -D warnings
   ```

4. Commit the registry, packages and catalogs. Pull requests run validation;
   merging to `main` publishes Pages.

The validator is pinned to a km003c-rs revision in `Cargo.toml`. Adopt parser
updates with `cargo add --git ... --rev ... km003c-lib`, regenerate the catalog
and lockfile, and review the results.

## Formats

`firmware.json` registers immutable paths, hashes, original filenames and source
IDs. `resources.json` records archive assets and provenance. `site/catalog.json`
and `site/resources.json` are generated and checked byte-for-byte in CI.

Catalog schema 1 has `model` and a `releases` array. Each release has a unique
`id`, `version`, `build_date`, `path`, absolute HTTPS `url`, `sha256`,
`package_size`, `firmware_size`, `crc32`, English/Chinese notes and provenance.
Use IDs or hashes when a version has multiple builds.

Manufacturer files retain their original notices and licenses. This repository
does not apply a new license to third-party applications or firmware.
