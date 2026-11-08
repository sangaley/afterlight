use rand::Rng;

/// Every surname a crew member can carry. The first twenty are the starting
/// roster in the order it has always been dealt; the rest came from the
/// hiring board's list and fresh additions.
///
/// The starter pincer has sixty berths and the old roster had twenty names,
/// so forty of a new captain's crew were "Hand 21" through "Hand 60" -- the
/// placeholder outnumbered the people. Bunks and the hiring board drew at
/// random from short lists of their own, which handed out names already
/// aboard.
pub const SURNAMES: [&str; 80] = [
    "Jones", "Smith", "Chen", "Morgan", "Rivera", "Volkov", "Tanaka", "Okafor",
    "Reyes", "Okonkwo", "Falk", "Ito", "Marsh", "Deng",
    "Ferrara", "Boone", "Ades", "Kowal", "Nyx", "Sorren",
    "Vega", "Osei", "Lindqvist", "Aoki", "Mercer", "Duval", "Ramaswamy", "Willow",
    "Stross", "Imani", "Costa", "Brun", "Ferro", "Solano", "Pike",
    "Abara", "Mbeki", "Novak", "Quist", "Haldane", "Ostrowski", "Saito", "Teague",
    "Adeyemi", "Brandt", "Castillo", "Duarte", "Eriksen", "Gallo", "Hargreaves",
    "Ibarra", "Kaur", "Laine", "Moreau", "Nakamura", "Petrov", "Quiroga", "Rask",
    "Sandoval", "Thorne", "Ueda", "Vos", "Whitlock", "Yilmaz", "Zhou", "Bello",
    "Cruz", "Esposito", "Fenn", "Grieve", "Holm", "Iqbal", "Kerr", "Lund",
    "Mwangi", "Orsini", "Rahimi", "Strand", "Yates", "Zeller",
];

/// The `i`th member of the starting crew. Numbered hands only past the end
/// of the pool, so two crew are never called the same thing.
pub fn starting_name(i: usize) -> String {
    match SURNAMES.get(i) {
        Some(name) => (*name).to_string(),
        None => format!("Hand {}", i + 1),
    }
}

/// A name nobody in `taken` already has, picked at random. Falls back to the
/// lowest free numbered hand once all eighty are spoken for.
pub fn unused_name<S: AsRef<str>>(taken: &[S], rng: &mut impl Rng) -> String {
    let in_use = |name: &str| taken.iter().any(|t| t.as_ref() == name);
    let free: Vec<&str> = SURNAMES.iter().copied().filter(|n| !in_use(n)).collect();
    if !free.is_empty() {
        return free[rng.gen_range(0..free.len())].to_string();
    }
    (1..)
        .map(|i| format!("Hand {i}"))
        .find(|n| !in_use(n))
        .expect("an unbounded range always has a free number")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn the_pool_has_no_repeats() {
        let unique: HashSet<_> = SURNAMES.iter().collect();
        assert_eq!(unique.len(), SURNAMES.len());
    }

    /// The starter pincer's sixty berths all get real names.
    #[test]
    fn a_full_starting_crew_is_all_named() {
        let names: Vec<String> = (0..60).map(starting_name).collect();
        assert!(names.iter().all(|n| !n.starts_with("Hand")), "{names:?}");
        assert_eq!(names.iter().collect::<HashSet<_>>().len(), 60);
    }

    #[test]
    fn a_new_hand_never_shares_a_name_with_someone_aboard() {
        let mut rng = rand::thread_rng();
        let mut aboard: Vec<String> = (0..60).map(starting_name).collect();
        for _ in 0..30 {
            let name = unused_name(&aboard, &mut rng);
            assert!(!aboard.contains(&name), "{name} is already aboard");
            aboard.push(name);
        }
        let unique: HashSet<_> = aboard.iter().collect();
        assert_eq!(unique.len(), aboard.len());
    }
}
