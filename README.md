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
cargo run -- --rootfs /path/to/rootfs --out /boot/initramfs.img
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

## See Also

Related SigmaOS **Core System** repositories:

- [SigmaOS-sigma-kernel](../SigmaOS-sigma-kernel/README.md) — SigmaOS microkernel / x86_64 kernel
- [SigmaOS-sigma-sh](../SigmaOS-sigma-sh/README.md) — SigmaOS interactive userspace shell
- [SigmaOS-sigma-sys](../SigmaOS-sigma-sys/README.md) — SigmaOS syscall ABI & userspace interface
- [SigmaOS-sigma-bootloader](../SigmaOS-sigma-bootloader/README.md) — SigmaOS bootloader
- [SigmaOS-installer](../SigmaOS-installer/README.md) — SigmaOS system installer
- [SigmaOS-ISO](../SigmaOS-ISO/README.md) — SigmaOS live & installable ISO builder
