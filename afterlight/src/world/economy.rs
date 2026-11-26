//! What a station has on its shelves, and what that does to its prices.
//!
//! Prices were a table: a station's type and a little jitter, the same all
//! game. Nothing ran short unless a timer said so, and selling a hundred
//! crystals fetched the same each as selling one. Now every station (and
//! starport) holds stock of each good. A station's normal level is where its
//! table price applies; less stock and it pays more, more and it pays less.
//!
//! Stock moves with trade: what you sell or buy, what traders deliver and
//! load (ai_ship::traffic), and now and then a shock. Left alone it drifts,
//! slowly, toward where the station would be with nobody trading: a station
//! that wants a good runs low on it, one that makes it piles it up. Traders
//! working the lanes are what keep prices near the table; lose the convoys
//! and the shortages come.

use bevy::prelude::*;

use crate::resources::ItemType;

/// Port index for a faction starport, past every station's index: its
/// prices are its own, not Haven's (which is what it used to fall back on).
pub const STARPORT_BASE: usize = crate::world::home_base::TOTAL_STATION_SLOTS;

/// The price index of a port.
pub fn port_index(port: crate::ai_ship::components::PortId) -> usize {
    use crate::ai_ship::components::PortId;
    match port {
        PortId::Station(i) => i,
        PortId::Starport(system) => STARPORT_BASE + system as usize,
    }
}

/// The goods, in the order the menus list them.
pub const GOODS: [ItemType; 7] = [
    ItemType::ScrapMetal,
    ItemType::Crystal,
    ItemType::BioSample,
    ItemType::FuelCell,
    ItemType::RareAlloy,
    ItemType::AncientArtifact,
    ItemType::AmmoCrate,
];

/// How much of a good an ordinary station keeps at its normal level.
fn depth(item: ItemType) -> f32 {
    match item {
        ItemType::ScrapMetal => 400.0,
        ItemType::FuelCell => 300.0,
        ItemType::AmmoCrate => 200.0,
        ItemType::Crystal => 150.0,
        ItemType::BioSample => 100.0,
        ItemType::RareAlloy => 80.0,
        ItemType::AncientArtifact => 16.0,
    }
}

/// A station's normal stock of a good: where its table price applies. A
/// trade hub, or a starport, keeps more of everything.
pub fn normal(idx: usize, item: ItemType) -> f32 {
    use crate::world::station_types::{station_type, StationType};
    let big = idx >= STARPORT_BASE || station_type(idx) == StationType::TradeHub;
    depth(item) * if big { 1.5 } else { 1.0 }
}

/// Where a station's stock of a good settles with nobody trading: low on
/// what it wants (it pays over the table for it), high on what it makes.
pub fn settles_at(idx: usize, item: ItemType) -> f32 {
    use crate::world::station_types::{station_type, type_price_multiplier};
    let wants = type_price_multiplier(station_type(idx), item);
    let lean = if wants > 1.05 { 0.6 } else if wants < 0.95 { 1.4 } else { 1.0 };
    normal(idx, item) * lean
}

/// How far stock and price can go: no station gives a good away or asks
/// the earth for it.
const PRICE_FLOOR: f32 = 0.5;
const PRICE_CEILING: f32 = 2.0;
/// How sharply price answers stock.
const ELASTICITY: f32 = 0.55;

/// What stock `s` of a normal `n` does to a price: 1 at the normal level,
/// more when short, less when there's plenty.
pub fn stock_factor(s: f32, n: f32) -> f32 {
    let r = (s / n.max(1.0)).max(0.01);
    r.powf(-ELASTICITY).clamp(PRICE_FLOOR, PRICE_CEILING)
}

/// Seconds for stock to go most of the way back to where it settles.
pub const DRIFT_S: f32 = 1_800.0;

/// `s` after `dt` seconds drifting toward `toward`. The same whether taken
/// in one step or many.
pub fn drift(s: f32, toward: f32, dt: f32) -> f32 {
    toward + (s - toward) * (-dt / DRIFT_S).exp()
}

/// A station keeps this share of its normal stock back: below it, it sells
/// no more ("they're keeping the rest").
pub const RESERVE: f32 = 0.1;

/// Every station's stock that has been touched; one never touched is at
/// its normal level. Kept on `MarketEvents`, saved with the world.
#[derive(Default, Clone, Debug)]
pub struct Stock {
    pub levels: std::collections::HashMap<(usize, ItemType), f32>,
}

impl Stock {
    pub fn get(&self, idx: usize, item: ItemType) -> f32 {
        self.levels.get(&(idx, item)).copied().unwrap_or_else(|| normal(idx, item))
    }

    /// The price multiplier stock gives a good at a station.
    pub fn factor(&self, idx: usize, item: ItemType) -> f32 {
        stock_factor(self.get(idx, item), normal(idx, item))
    }

    /// Put a port's every good on the books, at its normal level, if it
    /// isn't already: from then on it drifts like any other. A port nobody
    /// has seen stays at its table price, which is what the first visit
    /// finds.
    pub fn touch(&mut self, idx: usize) {
        for item in GOODS {
            self.levels.entry((idx, item)).or_insert_with(|| normal(idx, item));
        }
    }

    pub fn add(&mut self, idx: usize, item: ItemType, units: f32) {
        let s = self.get(idx, item);
        self.levels.insert((idx, item), (s + units).clamp(0.0, normal(idx, item) * 4.0));
    }

    /// How many a station will sell of a good before it keeps the rest.
    pub fn for_sale(&self, idx: usize, item: ItemType) -> f32 {
        (self.get(idx, item) - normal(idx, item) * RESERVE).max(0.0)
    }

    /// Short of it: paying well over the odds.
    pub fn short(&self, idx: usize, item: ItemType) -> bool {
        self.get(idx, item) < normal(idx, item) * 0.5
    }

    /// Plenty of it: selling it off cheap.
    pub fn plenty(&self, idx: usize, item: ItemType) -> bool {
        self.get(idx, item) > normal(idx, item) * 1.6
    }

    /// `dt` seconds of nobody trading, everywhere.
    pub fn drift_all(&mut self, dt: f32) {
        for ((idx, item), s) in self.levels.iter_mut() {
            *s = drift(*s, settles_at(*idx, *item), dt);
        }
    }

    pub fn to_saved(&self) -> Vec<(usize, ItemType, f32)> {
        let mut v: Vec<(usize, ItemType, f32)> = self.levels.iter().map(|((i, g), s)| (*i, *g, *s)).collect();
        v.sort_by(|a, b| (a.0, a.1 as u8).cmp(&(b.0, b.1 as u8)));
        v
    }

    pub fn from_saved(saved: &[(usize, ItemType, f32)]) -> Self {
        Stock { levels: saved.iter().map(|(i, g, s)| ((*i, *g), *s)).collect() }
    }
}

/// Seconds between drift steps.
const TICK_S: f32 = 5.0;

/// Stock drifts while you fly (the galaxy's clock runs only then too).
pub fn tick_economy(time: Res<Time>, mut next: Local<f32>, mut market: ResMut<crate::resources::MarketEvents>) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + TICK_S;
    if let Some(k) = econ_fast() {
        market.stock.drift_all(TICK_S * k);
    } else {
        market.stock.drift_all(TICK_S);
    }
}

/// AFTERLIGHT_ECON_FAST=<k>: stock drifts k times as fast (test switch).
fn econ_fast() -> Option<f32> {
    static V: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_ECON_FAST").ok().and_then(|v| v.parse().ok()))
}

/// AFTERLIGHT_ECON_TRACE=1: ECON lines for deals, deliveries and shocks.
pub fn econ_trace() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("AFTERLIGHT_ECON_TRACE").ok().as_deref() == Some("1"))
}

/// Stock goes into the world's save, and comes back out of a loaded one.
/// A new run's world (not yet populated) starts every station afresh.
pub fn mirror_stock(
    time: Res<Time>,
    mut next: Local<f32>,
    mut market: ResMut<crate::resources::MarketEvents>,
    mut sim: ResMut<crate::ai_ship::components::WorldSimulation>,
) {
    if sim.stock_loaded {
        sim.stock_loaded = false;
        market.stock = Stock::from_saved(&sim.stock);
        return;
    }
    if !sim.initialized {
        if !market.stock.levels.is_empty() {
            market.stock = Stock::default();
        }
        return;
    }
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 2.0;
    sim.stock = market.stock.to_saved();
}

/// Every so often a station's stock of something is hit: a fire in the
/// stores, a run on it, a glut. A port of the system you're in, at random.
pub fn stock_shocks(
    time: Res<Time>,
    mut next: Local<f32>,
    mut market: ResMut<crate::resources::MarketEvents>,
    stations: Res<crate::world::home_base::SystemStations>,
    ports: Res<crate::diplomacy::stations::FactionStations>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut notes: MessageWriter<crate::events::ShowNotification>,
) {
    use rand::Rng;
    let now = time.elapsed_secs();
    if *next == 0.0 {
        *next = now + 240.0;
    }
    if now < *next {
        return;
    }
    let mut rng = rand::thread_rng();
    *next = now + rng.gen_range(480.0..900.0);
    let mut here: Vec<(usize, String)> = stations.sites.iter().map(|s| (s.index, s.name.clone())).collect();
    if let Some(system) = streaming.loaded_system.filter(|id| ports.harbour(*id).is_some()) {
        here.push((STARPORT_BASE + system as usize, "the starport".to_string()));
    }
    if here.is_empty() {
        return;
    }
    let (idx, name) = here[rng.gen_range(0..here.len())].clone();
    let item = GOODS[rng.gen_range(0..GOODS.len())];
    let short = rng.gen_bool(0.65);
    let s = market.stock.get(idx, item);
    let to = if short { s * 0.3 } else { s.max(normal(idx, item)) * 1.8 };
    market.stock.levels.insert((idx, item), to);
    if econ_trace() {
        info!("ECON shock at {name}: {} {:.0} -> {:.0}", item.name(), s, to);
    }
    notes.write(crate::events::ShowNotification {
        message: if short {
            format!("MARKET: {name} has run short of {} - paying well for it", item.name())
        } else {
            format!("MARKET: {name} is overstocked with {} - selling it cheap", item.name())
        },
        notification_type: crate::events::NotificationType::Info,
        duration: 6.0,
    });
}

/// What a trader pays to load, over a station's price.
const WHOLESALE: f32 = 1.15;

/// How many units a trader with `haulers` working ships carries.
pub fn hold_of(haulers: usize) -> f32 {
    haulers as f32 * 40.0
}

/// The run a trader at port `from` makes next, among `to` (ports, and how
/// far): the good and the port that pay best for the time, with a little
/// noise so they don't all take the same one. `pledged` is what other
/// traders are already carrying to each port. None when nothing pays.
pub fn choose_run(
    stock: &Stock,
    from: usize,
    to: &[(usize, f32)],
    pledged: &std::collections::HashMap<(usize, ItemType), f32>,
    hold: f32,
    rng: &mut impl rand::Rng,
) -> Option<(usize, ItemType, f32)> {
    let mut best: Option<(f32, usize, ItemType, f32)> = None;
    for &(dest, distance) in to.iter().filter(|(d, _)| *d != from) {
        for item in GOODS {
            let coming = pledged.get(&(dest, item)).copied().unwrap_or(0.0);
            // No more than the port is short of: a full hold dumped where
            // they wanted a few left it glutted, and its price on the floor.
            let room = (normal(dest, item) * 1.1 - stock.get(dest, item) - coming).max(0.0);
            let units = hold.min(stock.for_sale(from, item)).min(room).floor();
            if units < 1.0 {
                continue;
            }
            // Wholesale: a trader loading a hold doesn't pay what you pay
            // for one crate at the counter.
            let base = crate::resources::shelf_base(from, item);
            let buy = base * stock.factor(from, item) * WHOLESALE;
            let there = stock.get(dest, item) + coming + units * 0.5;
            let sell = crate::resources::shelf_base(dest, item) * stock_factor(there, normal(dest, item));
            let margin = sell - buy;
            if margin <= 0.0 {
                continue;
            }
            let score = margin * units / (distance / 700.0 + 90.0) * rng.gen_range(0.75..1.25);
            if best.as_ref().is_none_or(|b| score > b.0) {
                best = Some((score, dest, item, units));
            }
        }
    }
    best.map(|(_, d, i, u)| (d, i, u))
}

#[cfg(test)]
mod economy_tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn day_one_prices_are_the_table() {
        let stock = Stock::default();
        for idx in [0, 3, 17, 40, STARPORT_BASE + 6] {
            for item in GOODS {
                assert_eq!(stock.factor(idx, item), 1.0, "{idx} {item:?}");
            }
        }
    }

    #[test]
    fn short_pays_more_plenty_pays_less_and_neither_without_end() {
        let n = 100.0;
        assert!(stock_factor(40.0, n) > 1.3 && stock_factor(250.0, n) < 0.7);
        assert!(stock_factor(10.0, n) > stock_factor(40.0, n));
        assert_eq!(stock_factor(0.0, n), PRICE_CEILING);
        assert_eq!(stock_factor(1.0e6, n), PRICE_FLOOR);
    }

    #[test]
    fn drift_is_the_same_in_one_step_or_many() {
        let one = drift(500.0, 100.0, 600.0);
        let mut many = 500.0;
        for _ in 0..120 {
            many = drift(many, 100.0, 5.0);
        }
        assert!((one - many).abs() < 0.01);
        assert!(drift(500.0, 100.0, DRIFT_S * 10.0) - 100.0 < 0.1);
    }

    /// Selling a lot walks the price down, and buying it back at the same
    /// counter always costs more than it fetched.
    #[test]
    fn selling_a_lot_walks_the_price_down_and_you_cant_buy_it_back_at_a_profit() {
        let mut stock = Stock::default();
        let idx = 5;
        let item = ItemType::Crystal;
        let before = stock.factor(idx, item);
        stock.add(idx, item, 120.0);
        assert!(stock.factor(idx, item) < before * 0.75);
        // Unit by unit: sell 30, buy 30.
        let mut stock = Stock::default();
        let (mut got, mut paid) = (0.0, 0.0);
        let base = crate::resources::shelf_base(idx, item);
        for _ in 0..30 {
            got += base * stock.factor(idx, item);
            stock.add(idx, item, 1.0);
        }
        for _ in 0..30 {
            stock.add(idx, item, -1.0);
            paid += base * stock.factor(idx, item) * 1.5;
        }
        assert!(paid > got, "bought back for {paid:.0} what fetched {got:.0}");
    }

    #[test]
    fn a_station_keeps_its_reserve() {
        let mut stock = Stock::default();
        let n = normal(4, ItemType::FuelCell);
        assert!((stock.for_sale(4, ItemType::FuelCell) - n * (1.0 - RESERVE)).abs() < 0.01);
        stock.add(4, ItemType::FuelCell, -n);
        assert_eq!(stock.for_sale(4, ItemType::FuelCell), 0.0);
    }

    /// Ten hours of a little economy: stations drifting, traders hauling by
    /// profit from where it's plentiful to where it's short. Prices stay in
    /// bounds; the traders keep them nearer the table than nobody would;
    /// and take the traders away and the wanting stations run short.
    #[test]
    fn ten_hours_of_trade_stay_sane() {
        let ports: Vec<usize> = (1..=8).collect();
        let distance = |a: usize, b: usize| ((a as f32 - b as f32).abs() * 60_000.0).max(40_000.0);
        let mut rng = rand::rngs::StdRng::seed_from_u64(9);
        let run = |traders: usize, hours: f32, rng: &mut rand::rngs::StdRng| -> Stock {
            let mut stock = Stock::default();
            for &p in &ports {
                stock.touch(p);
            }
            // (at, run: Some((dest, item, units, eta))).
            let mut fleets: Vec<(usize, Option<(usize, ItemType, f32, f32)>)> =
                (0..traders).map(|i| (ports[i % ports.len()], None)).collect();
            let dt = 30.0;
            let mut t = 0.0;
            while t < hours * 3600.0 {
                stock.drift_all(dt);
                let pledged: std::collections::HashMap<(usize, ItemType), f32> = {
                    let mut p = std::collections::HashMap::new();
                    for (_, r) in &fleets {
                        if let Some((d, i, u, _)) = r {
                            *p.entry((*d, *i)).or_insert(0.0) += u;
                        }
                    }
                    p
                };
                for f in fleets.iter_mut() {
                    match f.1 {
                        Some((dest, item, units, eta)) if eta <= t => {
                            stock.add(dest, item, units);
                            *f = (dest, None);
                        }
                        Some(_) => {}
                        None => {
                            let to: Vec<(usize, f32)> = ports.iter().map(|&p| (p, distance(f.0, p))).collect();
                            match choose_run(&stock, f.0, &to, &pledged, hold_of(3), rng) {
                                Some((dest, item, units)) => {
                                    stock.add(f.0, item, -units);
                                    f.1 = Some((dest, item, units, t + distance(f.0, dest) / 700.0 + 90.0));
                                }
                                None => {
                                    let dest = ports[rng.gen_range(0..ports.len())];
                                    f.1 = Some((dest, ItemType::ScrapMetal, 0.0, t + distance(f.0, dest) / 700.0 + 90.0));
                                }
                            }
                        }
                    }
                }
                t += dt;
            }
            stock
        };
        // How far prices sit from the table, on average (as a factor).
        let spread = |stock: &Stock| -> f32 {
            let (mut sum, mut n) = (0.0, 0.0);
            for &p in &ports {
                for item in GOODS {
                    let f = stock.factor(p, item);
                    assert!(f.is_finite() && f >= PRICE_FLOOR && f <= PRICE_CEILING);
                    let s = stock.get(p, item);
                    assert!(s >= 0.0 && s <= normal(p, item) * 4.0);
                    sum += f.ln().abs();
                    n += 1.0;
                }
            }
            sum / n
        };
        use rand::Rng;
        let traded = run(12, 10.0, &mut rng);
        let idle = run(0, 10.0, &mut rng);
        let (with, without) = (spread(&traded), spread(&idle));
        eprintln!("prices off the table on average: traded x{:.3}, idle x{:.3}", with.exp(), without.exp());
        assert!(with < without * 0.8, "trade didn't bring prices much nearer the table");
        // Nobody trading: the stations that want something run short of it.
        let wanting = ports.iter().flat_map(|&p| GOODS.iter().map(move |&g| (p, g))).find(|&(p, g)| settles_at(p, g) < normal(p, g));
        if let Some((p, g)) = wanting {
            assert!(idle.factor(p, g) > 1.2, "a station that wants {g:?} isn't short of it after ten idle hours");
        }
    }
}

