//! omc-xcmd CLI

use omc_xcmd::{executor, get_version, is_installed, skills};

fn main() {
    println!("omc-xcmd - x-cmd integration for OMC-RS\n");

    if !is_installed() {
        println!("❌ x-cmd not installed");
        println!("   Install: curl https://x-cmd.com | bash");
        std::process::exit(1);
    }

    println!("✅ x-cmd installed");

    if let Some(version) = get_version() {
        println!("   Version: {}", version);
    }

    let skill_count = skills::skill_count();
    println!("   Skills: {} installed", skill_count);

    let pkg_count = executor::count_packages().unwrap_or(0);
    println!("   Packages: {} installed", pkg_count);

    println!("\n--- Skills (first 10) ---");
    for skill in skills::list_skills().iter().take(10) {
        println!("  • {}", skill.name);
    }

    if skill_count > 10 {
        println!("  ... and {} more", skill_count - 10);
    }

    println!("\n--- Packages (first 10) ---");
    for pkg in executor::list_packages().iter().take(10) {
        println!("  • {}", pkg);
    }

    if pkg_count > 10 {
        println!("  ... and {} more", pkg_count - 10);
    }
}
