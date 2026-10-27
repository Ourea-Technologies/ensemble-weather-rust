// SPDX-License-Identifier: BSD-2-Clause

//! Records the compiling rustc's version for the User-Agent
//! (`ensemble-weather-rust/<version> (rustc/<version>)`, BEHAVIOR 2).

use std::process::Command;

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let version = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=ENSEMBLE_WEATHER_RUSTC_VERSION={version}");
    println!("cargo:rerun-if-env-changed=RUSTC");
}
