fn loading_screen_write(this: &Overlay<'_>, rel: &str) {
    // Only the game's own tables count: the mods' files are tracked per mod (reader.rs).
    if !JSON_TABLES.contains(&rel) && !NBT_TABLES.contains(&rel) {
        return;
    }
    let size = |name: &str| std::fs::metadata(this.base.join(name)).map_or(0, |m| m.len());
    if horizon_mods::content::loading_screen::snapshot().tables_count == 0 {
        let names: Vec<&str> = JSON_TABLES
            .iter()
            .chain(NBT_TABLES.iter())
            .copied()
            .filter(|n| this.base.join(n).exists())
            .collect();
        let bytes: u64 = names.iter().map(|n| size(n)).sum();
        horizon_mods::content::loading_screen::tables_known(names.len() as u32, bytes);
    }
    horizon_mods::content::loading_screen::table_written(size(rel));
}
