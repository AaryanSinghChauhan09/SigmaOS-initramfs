//! CLI for sigmaos-initramfs.

use clap::Parser;
use sigmaos_initramfs::pack::{pack_dir_compressed, Compression};
use sigmaos_initramfs::{plan_json, InitramfsConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sigmaos-initramfs", about = "SigmaOS initramfs hook pipeline")]
struct Cli {
    /// Config file (default: /etc/sigmaos/initramfs.toml).
    #[arg(short, long)]
    config: Option<PathBuf>,
    /// Print the resolved build plan as JSON.
    #[arg(long)]
    json: bool,
    /// Dry-run: show hook order without building.
    #[arg(long)]
    dry_run: bool,
    /// Pack a rootfs directory into a cpio initramfs image.
    #[arg(long)]
    rootfs: Option<PathBuf>,
    /// Output image path when using --rootfs (default: initramfs.cpio).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Compress the image: "none", "gzip", or "zstd" (default: none).
    #[arg(long, default_value = "none")]
    compress: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    if let Some(rootfs) = &cli.rootfs {
        let out = cli
            .out
            .clone()
            .unwrap_or_else(|| PathBuf::from("initramfs.cpio"));
        let compression: Compression = cli.compress.parse()?;
        let entries = pack_dir_compressed(rootfs, &out, compression)?;
        println!("wrote {entries} entries to {}", out.display());
        return Ok(());
    }

    let path = cli
        .config
        .unwrap_or_else(|| PathBuf::from("/etc/sigmaos/initramfs.toml"));

    let cfg = InitramfsConfig::load(&path)?;
    let plan = cfg.plan()?;

    if cli.json {
        println!("{}", plan_json(&plan)?);
        return Ok(());
    }

    if cli.dry_run {
        println!("image: {} ({})", plan.image.display(), plan.compression);
        if let Some(k) = &plan.kernel {
            println!("kernel: {k}");
        }
        for (i, s) in plan.steps.iter().enumerate() {
            println!(
                "  {}. {:<20} modules={} bins={} files={}",
                i + 1,
                s.hook,
                s.modules.len(),
                s.binaries.len(),
                s.files.len() + s.dirs.len()
            );
        }
        return Ok(());
    }

    // Real build would invoke sigma-initramfs here;
    // emit the plan for the caller to execute.
    println!("{}", plan_json(&plan)?);
    Ok(())
}
