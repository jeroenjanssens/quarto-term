use rand::Rng;

pub fn lognormal_ms(mean: f64, sigma: f64, rng: &mut impl Rng) -> u64 {
    let mu = mean.ln() - (sigma * sigma) / 2.0;
    let u1: f64 = rng.gen::<f64>().max(1e-10);
    let u2: f64 = rng.gen::<f64>();
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    let sample = (mu + sigma * z).exp();
    (sample as u64).max(10)
}

pub fn bigram_factor(prev: Option<char>, curr: char) -> f64 {
    match prev {
        None => 1.0,
        Some(p) => {
            if curr == ' ' { return 1.0; }
            if p == ' ' { return 1.3; }
            if p == '.' || p == ',' || p == ';' || p == ':' { return 1.8; }
            let same_hand = on_same_hand(p, curr);
            if p == curr { return 1.4; }
            if same_hand { 1.1 } else { 0.9 }
        }
    }
}

fn on_same_hand(a: char, b: char) -> bool {
    let left = "qwertasdfgzxcvb12345`~!@#$%";
    let a_left = left.contains(a.to_ascii_lowercase());
    let b_left = left.contains(b.to_ascii_lowercase());
    a_left == b_left
}

pub fn adjacent_key(ch: char, rng: &mut impl Rng) -> char {
    let neighbors: &[(char, &[char])] = &[
        ('a', &['s', 'q', 'z', 'w']),
        ('b', &['v', 'g', 'h', 'n']),
        ('c', &['x', 'd', 'f', 'v']),
        ('d', &['s', 'e', 'r', 'f', 'c', 'x']),
        ('e', &['w', 'r', 'd', 's', '3', '4']),
        ('f', &['d', 'r', 't', 'g', 'v', 'c']),
        ('g', &['f', 't', 'y', 'h', 'b', 'v']),
        ('h', &['g', 'y', 'u', 'j', 'n', 'b']),
        ('i', &['u', 'o', 'k', 'j', '8', '9']),
        ('j', &['h', 'u', 'i', 'k', 'm', 'n']),
        ('k', &['j', 'i', 'o', 'l', ',', 'm']),
        ('l', &['k', 'o', 'p', ';', '.', ',']),
        ('m', &['n', 'j', 'k', ',']),
        ('n', &['b', 'h', 'j', 'm']),
        ('o', &['i', 'p', 'l', 'k', '9', '0']),
        ('p', &['o', '[', ';', 'l', '0', '-']),
        ('q', &['w', 'a', '1', '2']),
        ('r', &['e', 't', 'f', 'd', '4', '5']),
        ('s', &['a', 'w', 'e', 'd', 'x', 'z']),
        ('t', &['r', 'y', 'g', 'f', '5', '6']),
        ('u', &['y', 'i', 'j', 'h', '7', '8']),
        ('v', &['c', 'f', 'g', 'b']),
        ('w', &['q', 'e', 's', 'a', '2', '3']),
        ('x', &['z', 's', 'd', 'c']),
        ('y', &['t', 'u', 'h', 'g', '6', '7']),
        ('z', &['a', 's', 'x']),
        ('1', &['2', 'q']),
        ('2', &['1', '3', 'q', 'w']),
        ('3', &['2', '4', 'w', 'e']),
        ('4', &['3', '5', 'e', 'r']),
        ('5', &['4', '6', 'r', 't']),
        ('6', &['5', '7', 't', 'y']),
        ('7', &['6', '8', 'y', 'u']),
        ('8', &['7', '9', 'u', 'i']),
        ('9', &['8', '0', 'i', 'o']),
        ('0', &['9', '-', 'o', 'p']),
        (' ', &[' ']),
    ];

    let lower = ch.to_ascii_lowercase();
    for (k, adj) in neighbors {
        if *k == lower {
            let picked = adj[rng.gen_range(0..adj.len())];
            if ch.is_uppercase() { return picked.to_ascii_uppercase(); }
            return picked;
        }
    }
    ch
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn lognormal_ms_minimum_floor() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            let val = lognormal_ms(50.0, 0.4, &mut rng);
            assert!(val >= 10);
        }
    }

    #[test]
    fn lognormal_ms_reasonable_range() {
        let mut rng = StdRng::seed_from_u64(123);
        let samples: Vec<u64> = (0..1000).map(|_| lognormal_ms(100.0, 0.4, &mut rng)).collect();
        let mean: f64 = samples.iter().map(|&x| x as f64).sum::<f64>() / 1000.0;
        assert!(mean > 50.0 && mean < 200.0);
    }

    #[test]
    fn bigram_factor_none_prev() {
        assert_eq!(bigram_factor(None, 'a'), 1.0);
    }

    #[test]
    fn bigram_factor_space_after_punct() {
        assert_eq!(bigram_factor(Some('.'), 'a'), 1.8);
        assert_eq!(bigram_factor(Some(','), 'b'), 1.8);
    }

    #[test]
    fn bigram_factor_same_char() {
        assert_eq!(bigram_factor(Some('a'), 'a'), 1.4);
    }

    #[test]
    fn bigram_factor_space_curr() {
        assert_eq!(bigram_factor(Some('a'), ' '), 1.0);
    }

    #[test]
    fn bigram_factor_space_prev() {
        assert_eq!(bigram_factor(Some(' '), 'a'), 1.3);
    }

    #[test]
    fn bigram_factor_same_hand() {
        assert_eq!(bigram_factor(Some('q'), 'w'), 1.1);
    }

    #[test]
    fn bigram_factor_cross_hand() {
        assert_eq!(bigram_factor(Some('f'), 'j'), 0.9);
    }

    #[test]
    fn adjacent_key_known_char() {
        let mut rng = StdRng::seed_from_u64(42);
        let result = adjacent_key('a', &mut rng);
        assert!(['s', 'q', 'z', 'w'].contains(&result));
    }

    #[test]
    fn adjacent_key_uppercase() {
        let mut rng = StdRng::seed_from_u64(42);
        let result = adjacent_key('A', &mut rng);
        assert!(result.is_uppercase());
    }

    #[test]
    fn adjacent_key_unknown() {
        let mut rng = StdRng::seed_from_u64(42);
        let result = adjacent_key('!', &mut rng);
        assert_eq!(result, '!');
    }
}
