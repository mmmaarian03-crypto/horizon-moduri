use horizon_cavedust::{
    Dust, Level, PARTICLES, Pos, Rng, SEA_LEVEL, Settings, WHITE_ASH, normalize, particle_amount,
    particle_name, pick, random_double,
};

/// Numbers from a list, then the last one for ever.
struct Fixed(Vec<f64>, usize);
impl Fixed {
    fn new(values: &[f64]) -> Self {
        Self(values.to_vec(), 0)
    }
}
impl Rng for Fixed {
    fn next_f64(&mut self) -> f64 {
        let v = self.0[self.1.min(self.0.len() - 1)];
        self.1 += 1;
        v
    }
}
/// A small xorshift, for the statistics.
struct Xorshift(u64);
impl Rng for Xorshift {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A cave: everything dark, solid air, overworld, unless a test says otherwise.
struct Cave {
    paused: bool,
    overworld: bool,
    bottom: i32,
    lush: bool,
    flat: bool,
    sky_above: i32,
    air: bool,
}
impl Default for Cave {
    fn default() -> Self {
        Self {
            paused: false,
            overworld: true,
            bottom: -64,
            lush: false,
            flat: false,
            sky_above: i32::MAX,
            air: true,
        }
    }
}
impl Level for Cave {
    fn paused(&self) -> bool {
        self.paused
    }
    fn bed_works(&self) -> bool {
        self.overworld
    }
    fn bottom_y(&self) -> i32 {
        self.bottom
    }
    fn lush_caves(&self, _: Pos) -> bool {
        self.lush
    }
    fn flat_world(&self) -> bool {
        self.flat
    }
    /// Sky light 15 from `sky_above` upwards.
    fn sky_visible(&self, pos: Pos) -> bool {
        pos[1] >= self.sky_above
    }
    fn is_air(&self, _: Pos) -> bool {
        self.air
    }
}

/// Fill the warm-up counter the way a few dark tries would.
fn warmed() -> Dust {
    Dust { timer: 10.0 }
}

#[test]
fn the_defaults_are_tuned_config() {
    let s = Settings::default();
    assert_eq!(s.dimension, 5);
    assert!(s.enabled && s.sea_level_check && !s.superflat_status);
    assert_eq!((s.upper_limit, s.lower_limit), (64, -64));
    assert_eq!((s.particle_multiplier, s.particle_multiplier_multiplier), (1, 10));
    assert_eq!(s.particle_id, 80);
}

#[test]
fn normalize_is_one_at_the_bottom_and_zero_at_the_top() {
    assert_eq!(normalize(-64.0, 64.0, -64.0), 1.0);
    assert_eq!(normalize(-64.0, 64.0, 64.0), 0.0);
    assert_eq!(normalize(-64.0, 64.0, 0.0), 0.5);
    // Outside the range it goes on.
    assert!(normalize(-64.0, 64.0, 100.0) < 0.0);
    assert!(normalize(-64.0, 64.0, -100.0) > 1.0);
}

#[test]
fn how_many_tries_per_tick_follow_the_depth() {
    let s = Settings::default();
    assert_eq!(particle_amount(&s, 64), 0);
    assert_eq!(particle_amount(&s, 63), 0, "0.078 cut to 0");
    assert_eq!(particle_amount(&s, 56), 0, "0.625 cut to 0");
    assert_eq!(particle_amount(&s, 50), 1);
    assert_eq!(particle_amount(&s, 30), 2);
    assert_eq!(particle_amount(&s, 0), 5);
    assert_eq!(particle_amount(&s, -30), 7);
    assert_eq!(particle_amount(&s, -64), 10);
    // Below the lower limit still more, above the upper limit never (negative, cut toward zero).
    assert_eq!(particle_amount(&s, -80), 11);
    assert_eq!(particle_amount(&s, 100), -2);
    assert_eq!(particle_amount(&s, 319), -19);
}

#[test]
fn the_multipliers_scale_the_tries() {
    let s = Settings {
        particle_multiplier: 3,
        particle_multiplier_multiplier: 20,
        ..Settings::default()
    };
    assert_eq!(particle_amount(&s, 0), 30);
    assert_eq!(particle_amount(&s, -64), 60);
}

#[test]
fn a_random_double_stays_inside_its_range() {
    assert_eq!(random_double(&mut Fixed::new(&[0.0]), -5.0, 5.0), -5.0);
    assert_eq!(random_double(&mut Fixed::new(&[0.5]), -5.0, 5.0), 0.0);
    let top = random_double(&mut Fixed::new(&[0.999_999_999_999_999_9]), -5.0, 5.0);
    assert!(top < 5.0 && top > 4.99);
    // A number that rounds up to the bound is brought back under it.
    let r = random_double(&mut Fixed::new(&[1.0]), -5.0, 5.0);
    assert!(r < 5.0);
    // ThreadLocalRandom refuses an empty range; the original would throw, this gives 0.
    assert_eq!(random_double(&mut Fixed::new(&[0.3]), 5.0, 5.0), 0.0);
}

#[test]
fn a_coordinate_is_cut_toward_zero_twice() {
    // Offset: (int) of a value in (-5, 5), so -4..=4 (and -5 only for exactly 0.0).
    assert_eq!(pick(&mut Fixed::new(&[0.0]), 100.0, 5), 95);
    assert_eq!(pick(&mut Fixed::new(&[0.05]), 100.0, 5), 96, "-4.5 cut to -4");
    assert_eq!(pick(&mut Fixed::new(&[0.5]), 100.0, 5), 100);
    assert_eq!(pick(&mut Fixed::new(&[0.95]), 100.0, 5), 104, "4.5 cut to 4");
    // A negative sum is cut toward zero, not floored: -3.5 is -3.
    assert_eq!(pick(&mut Fixed::new(&[0.5]), -3.5, 5), -3);
    assert_eq!(pick(&mut Fixed::new(&[0.5]), 3.7, 5), 3);
    // -3.5 plus an offset of +1 is -2.5, which is -2.
    assert_eq!(pick(&mut Fixed::new(&[0.6]), -3.5, 5), -2);
}

#[test]
fn tries_fall_within_the_box_around_the_player() {
    let s = Settings::default();
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    let mut dust = warmed();
    let feet = [10.4, -20.0, -7.6];
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..400 {
        for p in dust.tick(&s, &Cave::default(), &mut rng, feet) {
            seen.insert(p);
            assert!((6..=14).contains(&p[0]), "x {p:?}");
            assert!((-25..=-15).contains(&p[1]), "y {p:?}");
            assert!((-12..=-2).contains(&p[2]), "z {p:?}");
        }
    }
    // Plenty of different places, both ends reached.
    assert!(seen.len() > 250, "{}", seen.len());
    assert!(seen.iter().any(|p| p[0] == 6) && seen.iter().any(|p| p[0] == 14));
}

#[test]
fn the_offsets_are_not_uniform_because_zero_catches_both_signs() {
    // (int) of a value in (-1, 1) is 0: one try in five lands on the player's own coordinate.
    let mut rng = Xorshift(12345);
    let mut zeros = 0;
    let n = 20000;
    for _ in 0..n {
        if pick(&mut rng, 0.0, 5) == 0 {
            zeros += 1;
        }
    }
    let share = f64::from(zeros) / f64::from(n);
    assert!((0.17..0.23).contains(&share), "{share}");
}

#[test]
fn the_number_of_tries_follows_the_height_of_the_player() {
    let s = Settings::default();
    let mut rng = Xorshift(7);
    // At y = -30 a tick tries 7 places; all dark air, warmed up: 7 particles.
    assert_eq!(warmed().tick(&s, &Cave::default(), &mut rng, [0.5, -30.0, 0.5]).len(), 7);
    // Feet at -29.5 is block -30 as well; at -30.5 it is block -31 (7 tries too: 7.3).
    assert_eq!(warmed().tick(&s, &Cave::default(), &mut rng, [0.5, -29.5, 0.5]).len(), 7);
    assert_eq!(warmed().tick(&s, &Cave::default(), &mut rng, [0.5, -30.5, 0.5]).len(), 7);
    // At the surface nothing is tried.
    assert!(warmed().tick(&s, &Cave::default(), &mut rng, [0.5, 70.0, 0.5]).is_empty());
}

#[test]
fn nothing_spawns_until_the_counter_has_passed_ten() {
    let s = Settings::default();
    let cave = Cave::default();
    let mut dust = Dust::default();
    let at = [0, 0, 0];
    // Ten tries in the dark only count; the eleventh is the first particle.
    for i in 1..=10 {
        assert!(!dust.should_spawn(&s, &cave, at), "try {i}");
        assert_eq!(dust.timer, i as f32);
    }
    assert!(dust.should_spawn(&s, &cave, at));
    assert_eq!(dust.timer, 10.0, "kept at ten");
    assert!(dust.should_spawn(&s, &cave, at));
}

#[test]
fn a_refused_general_rule_clears_the_counter() {
    let s = Settings::default();
    let mut dust = warmed();
    let lush = Cave {
        lush: true,
        ..Cave::default()
    };
    assert!(!dust.should_spawn(&s, &lush, [0, 0, 0]));
    assert_eq!(dust.timer, 0.0);
    // The whole warm-up again afterwards.
    for _ in 0..10 {
        assert!(!dust.should_spawn(&s, &Cave::default(), [0, 0, 0]));
    }
    assert!(dust.should_spawn(&s, &Cave::default(), [0, 0, 0]));
}

#[test]
fn each_general_rule_clears_the_counter() {
    let s = Settings::default();
    let cases: Vec<(&str, Cave, Settings, Pos)> = vec![
        ("paused", Cave { paused: true, ..Cave::default() }, s.clone(), [0, 0, 0]),
        ("nether", Cave { overworld: false, ..Cave::default() }, s.clone(), [0, 0, 0]),
        ("below the world", Cave::default(), s.clone(), [0, -65, 0]),
        (
            "disabled",
            Cave::default(),
            Settings { enabled: false, ..s.clone() },
            [0, 0, 0],
        ),
    ];
    for (name, cave, settings, pos) in cases {
        let mut dust = warmed();
        assert!(!dust.should_spawn(&settings, &cave, pos), "{name}");
        assert_eq!(dust.timer, 0.0, "{name}");
    }
    // The bottom layer itself is allowed (`bottomY > y` refuses, equal does not).
    let mut dust = warmed();
    assert!(dust.should_spawn(&s, &Cave::default(), [0, -64, 0]));
}

#[test]
fn sky_and_height_do_not_clear_the_counter_they_only_refuse() {
    let s = Settings::default();
    let mut dust = warmed();
    // Sky visible: refused, counter untouched.
    let open = Cave {
        sky_above: 0,
        ..Cave::default()
    };
    assert!(!dust.should_spawn(&s, &open, [0, 10, 0]));
    assert_eq!(dust.timer, 10.0);
    // Too high (y + 2 must be below sea level): refused, untouched.
    assert!(!dust.should_spawn(&s, &Cave::default(), [0, SEA_LEVEL - 2, 0]));
    assert_eq!(dust.timer, 10.0);
    assert!(dust.should_spawn(&s, &Cave::default(), [0, SEA_LEVEL - 3, 0]));
}

#[test]
fn the_sea_level_limit_is_y_plus_two_below_63() {
    let s = Settings::default();
    let cave = Cave::default();
    assert!(warmed().should_spawn(&s, &cave, [0, 60, 0]));
    assert!(!warmed().should_spawn(&s, &cave, [0, 61, 0]));
    assert!(!warmed().should_spawn(&s, &cave, [0, 62, 0]));
}

#[test]
fn the_sea_level_check_option_changes_nothing() {
    // The original defines `seaLevelCheck` and never reads it.
    let on = Settings::default();
    let off = Settings {
        sea_level_check: false,
        ..Settings::default()
    };
    for y in [-60, 0, 60, 61, 70] {
        assert_eq!(
            warmed().should_spawn(&on, &Cave::default(), [0, y, 0]),
            warmed().should_spawn(&off, &Cave::default(), [0, y, 0]),
            "{y}"
        );
    }
}

#[test]
fn the_flat_world_rule_only_applies_when_the_option_is_on() {
    // In the original `superFlatStatus = false` skips the flat check altogether.
    let flat = Cave {
        flat: true,
        ..Cave::default()
    };
    assert!(warmed().should_spawn(&Settings::default(), &flat, [0, 0, 0]));
    let on = Settings {
        superflat_status: true,
        ..Settings::default()
    };
    let mut dust = warmed();
    assert!(!dust.should_spawn(&on, &flat, [0, 0, 0]));
    assert_eq!(dust.timer, 10.0, "refused without clearing the counter");
    assert!(dust.should_spawn(&on, &Cave::default(), [0, 0, 0]));
}

#[test]
fn only_air_gets_dust_but_every_try_counts() {
    let s = Settings::default();
    let mut rng = Xorshift(99);
    let stone = Cave {
        air: false,
        ..Cave::default()
    };
    let mut dust = Dust::default();
    // Seven solid-block tries (at y = -30) still warm the counter up.
    let out = dust.tick(&s, &stone, &mut rng, [0.5, -30.0, 0.5]);
    assert!(out.is_empty());
    assert_eq!(dust.timer, 7.0);
    // Then the next seven: three more warm it to 10, four are dust.
    let out = dust.tick(&s, &Cave::default(), &mut rng, [0.5, -30.0, 0.5]);
    assert_eq!(out.len(), 4);
}

#[test]
fn a_fresh_cave_needs_a_second_tick_before_the_first_dust() {
    let s = Settings::default();
    let mut rng = Xorshift(5);
    let mut dust = Dust::default();
    // At y = 0 five tries per tick: the counter reaches 10 in two ticks, and the 11th try is the
    // first dust, so the third tick has the first particles.
    let first = dust.tick(&s, &Cave::default(), &mut rng, [0.5, 0.0, 0.5]);
    let second = dust.tick(&s, &Cave::default(), &mut rng, [0.5, 0.0, 0.5]);
    let third = dust.tick(&s, &Cave::default(), &mut rng, [0.5, 0.0, 0.5]);
    assert!(first.is_empty() && second.is_empty());
    assert_eq!(third.len(), 5, "the 11th try is the first dust, and it opens the third tick");
}

#[test]
fn the_particle_id_of_the_config_is_white_ash() {
    assert_eq!(PARTICLES.len(), 95);
    assert_eq!(PARTICLES[80], "white_ash");
    assert_eq!(particle_name(Settings::default().particle_id), "white_ash");
    assert_eq!(PARTICLES[29], "cherry_leaves", "1.20 put leaves in at 29");
    assert_eq!(PARTICLES[4], "bubble");
}

#[test]
fn another_id_picks_another_simple_particle() {
    assert_eq!(particle_name(0), "ambient_entity_effect");
    assert_eq!(particle_name(5), "cloud");
    assert_eq!(particle_name(50), "rain");
    assert_eq!(particle_name(94), "egg_crack");
}

#[test]
fn an_id_that_cannot_be_used_falls_back_to_white_ash() {
    // Out of the registry.
    assert_eq!(particle_name(95), WHITE_ASH);
    assert_eq!(particle_name(-1), WHITE_ASH);
    assert_eq!(particle_name(1000), WHITE_ASH);
    // Types that need options (the original's ClassCastException).
    for id in [2, 3, 14, 15, 25, 31, 40, 41, 93] {
        assert_eq!(particle_name(id), WHITE_ASH, "{} ({id})", PARTICLES[id as usize]);
    }
}

#[test]
fn the_particle_names_are_unique() {
    let mut names: Vec<&str> = PARTICLES.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), PARTICLES.len());
}
