# SigmaOS Initramfs

Initramfs builder with `cpio`/`pack` image generation plus a
mkinitcpio-style hook pipeline for kernel modules, binaries, and files.

## Capabilities

- `newc` cpio archive writer and deterministic directory-tree packer
- TOML hook definitions with `depends` and `order`
- Deterministic topological ordering (Kahn's algorithm)
- Cycle and missing-dependency detection
- JSON build plan for `sigmaos-rebuild` to execute
- `--dry-run` / `--json` CLI output
- `--rootfs <dir> --out <img>` packs a directory into a cpio `newc` image

## Interconnectivity

| Repo | Relationship |
|------|-------------|
| `sigmaos-rebuild` | Consumes initramfs build plans during rebuild |
| `sigmaos-dkms` | DKMS modules feed hook `modules` lists |
| `sigmaos-ISO` | ISO build embeds the resulting image |

## Status

Implemented and pushed to `SigmaOS-initramfs/main` (merged with the
canonical cpio/pack history; 17 tests passing).
