//! `freeblue` CLI — thin front-end over `freeblue-core` (spec 08 §8.4).
//!
//! Planned subcommands:
//!   freeblue decrypt <disc> --keydb <KEYDB.cfg> -o <out.m2ts>
//!   freeblue verify  <disc> --reference <makemkv.m2ts>   (spec 09 §9.3)

use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("decrypt") => {
            // TODO(TDD): wire freeblue_core::decrypt_clip once disc/keys land.
            anyhow::bail!("`decrypt` not yet implemented — see specs/08 + roadmap")
        }
        Some("verify") => {
            anyhow::bail!("`verify` not yet implemented — see specs/09 §9.3")
        }
        _ => {
            eprintln!("freeblue {}", env!("CARGO_PKG_VERSION"));
            eprintln!("usage: freeblue <decrypt|verify> ...  (not yet implemented)");
            Ok(())
        }
    }
}
