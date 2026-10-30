fn prepare(
    base: &std::path::Path,
    mods: &[crate::mod_content::ContentMod],
    cache: &std::path::Path,
) -> core::ops::ControlFlow<Result<PathBuf>> {
    if !mods.iter().any(|m| m.id == "create") {
        return core::ops::ControlFlow::Continue(());
    }
    core::ops::ControlFlow::Break((|| {
        let out = cache.join("mod-content");
        let fingerprint = stamp(base, mods)?;
        if std::fs::read(out.join(STAMP)).is_ok_and(|s| s == fingerprint.as_bytes()) {
            let warnings = std::fs::read_to_string(out.join(WARNINGS)).unwrap_or_default();
            warnings.lines().for_each(|w| log::warn!("Horizon: {w}"));
            return Ok(out);
        }
        for warning in build(base, mods, &out)? {
            log::warn!("Horizon: {warning}");
        }
        create_preview_support::apply(&out)?;
        std::fs::write(out.join(STAMP), fingerprint)?;
        Ok(out)
    })())
}
