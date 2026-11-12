//! Discovers the League installs a machine holds, as the manager would.

use vinciskin_cn::install::tencent::{TencentInstallSource, WinRegistryProbe};
use vinciskin_cn::install::InstallSource;

fn main() {
    let source = TencentInstallSource::new(WinRegistryProbe);
    let found = source.discover();

    println!("discovered {} patchline(s)", found.len());
    for patchline in &found {
        let game = patchline.root.join("Game");
        println!("  id       = {}", patchline.id);
        println!("  root     = {}", patchline.root.display());
        println!("  Game dir = {}", game.exists());
        println!(
            "  exe      = {}",
            game.join("League of Legends.exe").exists()
        );
        println!("  DATA     = {}", game.join("DATA").exists());
    }
}
