fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        moleapi_script_runtime::dispatch_worker()?,
        "Private script worker mode required"
    );
    Ok(())
}
