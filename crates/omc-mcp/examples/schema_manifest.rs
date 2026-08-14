fn main() -> Result<(), serde_json::Error> {
    println!(
        "{}",
        serde_json::to_string_pretty(&omc_mcp::schema_contract::current_manifest())?
    );
    Ok(())
}
