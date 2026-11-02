fn clumps_orb_save(tag: Tag, o: &ExperienceOrb) -> Tag {
    let mut tag = tag;
    if o.value > i16::MAX as u32 {
        let (size, count) = clumps_save::split_value(o.value);
        put(&mut tag, "Value", Tag::Short(size as i16));
        put(&mut tag, "Count", Tag::Int(count.min(i32::MAX as u32) as i32));
    }
    tag
}
