use horizon_waterdripsound::*;

fn tuned() -> Settings {
    Settings::tuned()
}

#[test]
fn water_drip_on_dry_ground_plays_the_dripstone_water_sound() {
    let s = land_sound(&tuned(), Drip::Water, Fluid::Empty, 0.5).unwrap();
    assert_eq!(s.event, DRIP_WATER);
    assert_eq!(s.pitch, 1.0);
    // 0.3f * (0.5 * 0.7 + 0.3)
    assert_eq!(s.volume, (f64::from(0.3f32) * 0.65) as f32);
}

#[test]
fn water_drip_into_water_is_silent() {
    for f in [Fluid::Water, Fluid::FlowingWater, Fluid::Lava, Fluid::FlowingLava] {
        assert_eq!(land_sound(&tuned(), Drip::Water, f, 0.5), None, "{f:?}");
    }
}

#[test]
fn lava_drip_is_silent_only_on_a_lava_source() {
    assert_eq!(land_sound(&tuned(), Drip::Lava, Fluid::Lava, 0.1), None);
    for f in [Fluid::Empty, Fluid::Water, Fluid::FlowingWater, Fluid::FlowingLava] {
        assert_eq!(land_sound(&tuned(), Drip::Lava, f, 0.1).unwrap().event, DRIP_LAVA, "{f:?}");
    }
}

#[test]
fn volume_spread_is_thirty_to_hundred_percent() {
    let lo = land_sound(&tuned(), Drip::Water, Fluid::Empty, 0.0).unwrap().volume;
    let hi = land_sound(&tuned(), Drip::Water, Fluid::Empty, 0.999_999).unwrap().volume;
    assert!((lo - 0.09).abs() < 1e-6, "{lo}");
    assert!((hi - 0.3).abs() < 1e-5, "{hi}");
}

#[test]
fn without_dripstone_sounds_the_volume_is_flat() {
    let mut s = tuned();
    s.use_dripstone_sounds = false;
    assert_eq!(land_sound(&s, Drip::Water, Fluid::Empty, 0.0).unwrap().volume, 0.3);
    s.volume = 2.5;
    assert_eq!(land_sound(&s, Drip::Water, Fluid::Empty, 0.0).unwrap().volume, 1.0);
}

#[test]
fn disabled_plays_nothing() {
    let mut s = tuned();
    s.enabled = false;
    assert_eq!(land_sound(&s, Drip::Water, Fluid::Empty, 0.5), None);
}

#[test]
fn below_truncates_like_java_int_casts() {
    assert_eq!(below([3.7, 64.0, 5.2]), [3, 63, 5]);
    // (int)-3.5 == -3, not floor -4.
    assert_eq!(below([-3.5, 10.2, -0.4]), [-3, 9, 0]);
    assert_eq!(below([0.0, -0.5, 0.0]), [0, -1, 0]);
}

#[test]
fn fluid_types_from_facts() {
    assert_eq!(Fluid::of(false, false, 0, false), Fluid::Empty);
    assert_eq!(Fluid::of(true, false, 8, false), Fluid::Water);
    assert_eq!(Fluid::of(true, false, 8, true), Fluid::FlowingWater);
    assert_eq!(Fluid::of(true, false, 5, false), Fluid::FlowingWater);
    assert_eq!(Fluid::of(false, true, 8, false), Fluid::Lava);
    assert_eq!(Fluid::of(false, true, 3, false), Fluid::FlowingLava);
}

#[test]
fn tuned_values_leave_the_vanilla_constants_alone() {
    // dripChance 10 and both multipliers 1.0: the @ModifyConstant hooks change nothing, so the port has none.
    assert_eq!(config::DRIP_CHANCE, vanilla::DRIP_CHANCE);
    assert_eq!(scaled(vanilla::ANIMATE_COUNT, config::BLOCK_ANIMATE_COUNT_MULTIPLIER), 667);
    assert_eq!(scaled(vanilla::ANIMATE_RANGE_1, config::BLOCK_ANIMATE_RANGE_MULTIPLIER), 16);
    assert_eq!(scaled(vanilla::ANIMATE_RANGE_2, config::BLOCK_ANIMATE_RANGE_MULTIPLIER), 32);
    assert_eq!(add_range(config::BLOCK_ANIMATE_RANGE_MULTIPLIER), 1024.0);
    assert_eq!(scaled(667, 2.5), 1667);
    assert_eq!(add_range(0.5), 1024.0);
}

#[test]
fn settings_cycle_like_the_screen() {
    let s = tuned();
    assert_eq!(s.volume_percent(), 30);
    assert_eq!(s.category_index(), 8);
    assert_eq!(SOURCES[s.category_index()], "ambient");
    assert_eq!(next_volume_percent(30), 40);
    assert_eq!(next_volume_percent(95), 100);
    assert_eq!(next_volume_percent(100), 0);
    assert_eq!(next_category("ambient"), "voice");
    assert_eq!(next_category("voice"), "master");
    assert_eq!(next_category("nonsense"), "voice");
}
