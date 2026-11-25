# SigmaOS-initramfs

Initramfs builder with two layers:

- `cpio` / `pack` — `newc` archive writer and directory-tree packer
  (mkinitcpio/archiso-style image generation).
- Hook pipeline — TOML hook definitions with `depends`/`order`,
  deterministic topological ordering, cycle/missing-dependency
  detection, and JSON build plans for `sigmaos-rebuild`.

## Usage

```sh
cargo run -- --config /etc/sigmaos/initramfs.toml --dry-run
cargo run -- --config initramfs.toml --json
```

## Config format

```toml
image = "/boot/initramfs-6.1.img"
compression = "zstd"
kernel = "6.1.0"

[[hooks]]
name = "base"
order = 10
[hooks.payload]
binaries = ["busybox", "modprobe"]

[[hooks]]
name = "storage"
depends = ["base"]
order = 20
[hooks.payload]
modules = ["nvme", "xhci_hcd", "ext4"]
```

## Guarantees

- Deterministic hook ordering (topo sort + order/name tie-break)
- Cycle and missing-dependency detection with named errors
- `#![forbid(unsafe_code)]`; fail-fast errors; GPL-3.0-or-later

Part of the SigmaOS multi-repository component ecosystem.
