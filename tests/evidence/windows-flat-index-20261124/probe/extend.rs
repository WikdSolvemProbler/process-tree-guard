use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

#[path = "baseline/rate.rs"]
mod baseline;
#[path = "candidate/rate.rs"]
mod candidate;

type Key = (u32, u64);
const RATE_LIMIT: usize = 12;
const RATE_WINDOW_MS: u64 = 10_000;
const MAX_RATE_KEYS: usize = 1024;
const FORGET_MS: u64 = 250;
const REPEATS: u64 = 32;
const CASES: [(&str, u64); 12] = [
    ("hit-refund-1", 8192), ("hit-refund-32", 8192), ("hit-refund-1024", 8192),
    ("fill-ascending", REPEATS * 1024), ("fill-descending", REPEATS * 1024),
    ("first-full-miss", REPEATS), ("backed-off-miss", REPEATS * 256),
    ("one-dead-churn", 128), ("mostly-retaining-expiry", REPEATS),
    ("alternating-retain", REPEATS), ("full-expiry", REPEATS), ("shutdown", REPEATS),
];

struct Process { created: u64 }

impl Process {
    fn open(_: u32) -> Option<Self> { panic!("native Process::open was reached") }
    fn alive(&self) -> bool { panic!("native Process::alive was reached") }
}

#[derive(Debug, Eq, PartialEq)]
enum Outcome { Allowed, First, Counted, Full }

trait Table: Default {
    fn admit(&mut self, key: Key, now: u64, alive: &dyn Fn(Key) -> bool) -> Outcome;
    fn refund(&mut self, key: Key);
    fn unrefuse(&mut self, key: Key);
    fn retain(&mut self, keep: &dyn Fn(Key) -> bool);
    fn sweep(&mut self, now: u64, all: bool);
    fn len(&self) -> usize;
    fn rate(&self, position: usize) -> (Key, &[u64], u32);
    fn summaries(&self) -> Vec<(u32, u32)>;
    fn unproductive(&self) -> Option<u64>;
    fn next_close(&self) -> Option<u64>;
}

macro_rules! table {
    ($arm:ident) => {
        impl Table for $arm::Limiter {
            fn admit(&mut self, key: Key, now: u64, alive: &dyn Fn(Key) -> bool) -> Outcome {
                match $arm::Limiter::admit(self, key, (key.0, "probe.exe"), now, alive) {
                    $arm::Admit::Allowed => Outcome::Allowed,
                    $arm::Admit::First => Outcome::First,
                    $arm::Admit::Counted => Outcome::Counted,
                    $arm::Admit::Full => Outcome::Full,
                }
            }
            fn refund(&mut self, key: Key) { $arm::Limiter::refund(self, key); }
            fn unrefuse(&mut self, key: Key) { $arm::Limiter::unrefuse(self, key); }
            fn retain(&mut self, keep: &dyn Fn(Key) -> bool) {
                $arm::Limiter::retain(self, |rate| keep(rate.key));
            }
            fn sweep(&mut self, now: u64, all: bool) { $arm::Limiter::sweep(self, now, all); }
            fn len(&self) -> usize { self.rates.len() }
            fn rate(&self, position: usize) -> (Key, &[u64], u32) {
                let rate = &self.rates[position];
                let image = if rate.key == (0, 0) { "outside the job" } else { "probe.exe" };
                assert_eq!((rate.pid, rate.image.as_str()), (rate.key.0, image));
                (rate.key, &rate.times, rate.refused)
            }
            fn summaries(&self) -> Vec<(u32, u32)> {
                self.summaries.iter().map(|row| {
                    assert_eq!(row.image, if row.pid == 0 { "outside the job" } else { "probe.exe" });
                    (row.pid, row.count)
                }).collect()
            }
            fn unproductive(&self) -> Option<u64> { self.unproductive }
            fn next_close(&self) -> Option<u64> { self.next_close }
        }
    };
}

table!(baseline);
table!(candidate);

fn key(pid: u32) -> Key { (pid, u64::from(pid) + 10) }

fn keys(count: usize) -> Vec<Key> {
    (0..count).map(|n| key(10_000 + n as u32)).collect()
}

fn setup<T: Table>(keys: &[Key], stamp: impl Fn(usize) -> u64) -> T {
    let mut table = T::default();
    for (position, &key) in keys.iter().enumerate() {
        assert_eq!(table.admit(key, stamp(position), &|_| true), Outcome::Allowed);
    }
    table
}

fn order<T: Table>(table: &T, expected: &[Key]) -> BTreeMap<Key, usize> {
    assert_eq!(table.len(), expected.len());
    let positions: BTreeMap<_, _> = expected.iter().copied().enumerate().map(|(i,k)| (k,i)).collect();
    assert_eq!(positions.len(), expected.len(), "oracle keys must be unique");
    for (i, wanted) in expected.iter().enumerate() {
        let (actual, _, _) = table.rate(i);
        assert_eq!(&actual, wanted);
        assert_eq!(positions.get(&actual), Some(&i));
    }
    positions
}

fn verify_index<T: Table>(table: &mut T, expected: &[Key], now: u64) {
    for (&key, &position) in &order(table, expected) {
        let before = table.rate(position).1.to_vec();
        assert_eq!(table.admit(key, now, &|_| panic!("known key asked for a liveness scan")), Outcome::Allowed);
        let after = table.rate(position).1;
        assert_eq!(&after[..before.len()], before.as_slice());
        assert_eq!(after.len(), before.len() + 1);
        assert_eq!(after.last(), Some(&now));
        assert_eq!(table.len(), expected.len());
        table.refund(key);
        assert_eq!(table.rate(position).1, before.as_slice());
    }
}

fn empty<T: Table>(table: &mut T, now: u64) {
    assert_eq!(table.len(), 0);
    assert_eq!(table.next_close(), None);
    let reused = key(10_000);
    assert_eq!(table.admit(reused, now, &|_| true), Outcome::Allowed);
    assert_eq!(table.rate(0), (reused, &[now][..], 0));
}

fn timed<T>(operation: impl FnOnce() -> T) -> (T, u128) {
    let start = Instant::now();
    let result = black_box(operation());
    let elapsed = Instant::now().checked_duration_since(start).expect("clock went backwards").as_nanos();
    assert!(elapsed > 0, "timer gave zero interval");
    (result, elapsed)
}

fn hit<T: Table>(count: usize) -> u128 {
    let keys = keys(count);
    let mut table = setup::<T>(&keys, |_| 1);
    let (allowed, ns) = timed(|| {
        let mut allowed = 0;
        for n in 0..8192 {
            let key = black_box(keys[(641 * n) % count]);
            allowed += u64::from(table.admit(key, 1, &|_| true) == Outcome::Allowed);
            table.refund(key);
        }
        allowed
    });
    assert_eq!(allowed, 8192);
    verify_index(&mut table, &keys, 1);
    assert!(table.summaries().is_empty());
    ns
}

fn fill<T: Table>(descending: bool) -> u128 {
    let mut keys = keys(MAX_RATE_KEYS);
    if descending { keys.reverse(); }
    (0..REPEATS).map(|_| {
        let mut table = T::default();
        let (allowed, ns) = timed(|| {
            keys.iter().filter(|&&key| table.admit(black_box(key), 1, &|_| true) == Outcome::Allowed).count()
        });
        assert_eq!(allowed, MAX_RATE_KEYS);
        verify_index(&mut table, &keys, 1);
        ns
    }).sum()
}

fn misses<T: Table>() -> (u128, u128) {
    let keys = keys(MAX_RATE_KEYS);
    let unknown = key(8_000);
    let (mut first_total, mut backoff_total) = (0, 0);
    for _ in 0..REPEATS {
        let mut table = setup::<T>(&keys, |_| 1);
        let scans = std::cell::Cell::new(0);
        let all_alive = |_| { scans.set(scans.get() + 1); true };
        let (first, ns) = timed(|| table.admit(black_box(unknown), 2, &all_alive));
        first_total += ns;
        assert_eq!(first, Outcome::Full);
        assert_eq!(scans.get(), MAX_RATE_KEYS);
        assert_eq!(table.unproductive(), Some(2));
        let (full, ns) = timed(|| {
            (0..256).filter(|_| table.admit(black_box(unknown), 3, &all_alive) == Outcome::Full).count()
        });
        backoff_total += ns;
        assert_eq!(full, 256);
        assert_eq!(table.admit(unknown, 251, &all_alive), Outcome::Full);
        assert_eq!(scans.get(), MAX_RATE_KEYS);
        assert_eq!(table.admit(unknown, 2 + FORGET_MS, &all_alive), Outcome::Full);
        assert_eq!(scans.get(), 2 * MAX_RATE_KEYS);
        assert_eq!(table.unproductive(), Some(2 + FORGET_MS));
        verify_index(&mut table, &keys, 2 + FORGET_MS);
    }
    (first_total, backoff_total)
}

fn churn<T: Table>() -> u128 {
    let keys = keys(MAX_RATE_KEYS);
    let mut table = setup::<T>(&keys, |_| 1);
    let dead: Vec<_> = keys.iter().copied().take(128).collect();
    let new: Vec<_> = (0..128).map(|n| key(9_999 - n)).collect();
    let expected: Vec<_> = keys[128..].iter().chain(&new).copied().collect();
    let ((allowed, polls), ns) = timed(|| {
        let mut allowed = 0;
        let polls = std::cell::Cell::new(0);
        for (n, (&dead, &new)) in dead.iter().zip(&new).enumerate() {
            let alive = |key| { polls.set(polls.get() + 1); key != dead };
            allowed += u64::from(table.admit(black_box(new), 2 + n as u64, &alive) == Outcome::Allowed);
        }
        (allowed, polls.get())
    });
    assert_eq!(allowed, 128);
    assert_eq!(polls, 128 * MAX_RATE_KEYS);
    assert_eq!(table.unproductive(), None);
    verify_index(&mut table, &expected, 129);
    ns
}

fn expiry<T: Table>(mostly_keep: bool) -> u128 {
    let keys = keys(MAX_RATE_KEYS);
    (0..REPEATS).map(|_| {
        let mut table = setup::<T>(&keys, |i| u64::from(mostly_keep && i != 0));
        let (_, ns) = timed(|| table.sweep(black_box(10_000), false));
        assert!(table.summaries().is_empty());
        if mostly_keep {
            assert_eq!(table.next_close(), Some(10_001));
            verify_index(&mut table, &keys[1..], 10_000);
            assert_eq!(table.admit(keys[0], 10_000, &|_| true), Outcome::Allowed);
            assert_eq!(table.rate(MAX_RATE_KEYS - 1).0, keys[0]);
        } else {
            empty(&mut table, 10_000);
        }
        ns
    }).sum()
}

fn half_retain<T: Table>() -> u128 {
    let keys = keys(MAX_RATE_KEYS);
    let expected: Vec<_> = keys.iter().copied().step_by(2).collect();
    (0..REPEATS).map(|_| {
        let mut table = setup::<T>(&keys, |_| 1);
        let keep = |key: Key| black_box(key.0) % 2 == 0;
        let (_, ns) = timed(|| table.retain(&keep));
        verify_index(&mut table, &expected, 1);
        assert!(table.summaries().is_empty());
        ns
    }).sum()
}

fn counted<T: Table>(keys: &[Key]) -> T {
    let mut table = T::default();
    for &key in keys {
        for _ in 0..RATE_LIMIT {
            assert_eq!(table.admit(key, 1, &|_| true), Outcome::Allowed);
        }
        assert_eq!(table.admit(key, 1, &|_| true), Outcome::First);
        assert_eq!(table.admit(key, 1, &|_| true), Outcome::Counted);
    }
    table
}

fn shutdown<T: Table>() -> u128 {
    let initial = keys(MAX_RATE_KEYS);
    let keys: Vec<_> = (0..MAX_RATE_KEYS).map(|i| initial[(i * 641) % MAX_RATE_KEYS]).collect();
    let expected: Vec<_> = keys.iter().map(|k| (k.0, 1)).collect();
    (0..REPEATS).map(|_| {
        let mut table = counted::<T>(&keys);
        let (_, ns) = timed(|| table.sweep(black_box(2), true));
        assert_eq!(table.summaries(), expected);
        empty(&mut table, 2);
        ns
    }).sum()
}

fn oracle<T: Table>() {
    let original = [(19, 32), (3, 5), (19, 9), (0, 0), (2, 1)];
    let mut table = counted::<T>(&original);
    let survivors = &original[1..4];
    table.retain(&|key| survivors.contains(&key));
    let positions = order(&table, survivors);
    assert_eq!(table.summaries(), [(19, 1), (2, 1)]);
    table.unrefuse((19, 9));
    assert_eq!(table.rate(positions[&(19, 9)]).2, 1);
    table.refund((3, 5));
    assert_eq!(table.rate(positions[&(3, 5)]).1.len(), RATE_LIMIT - 1);
    assert_eq!(table.admit((19, 32), 2, &|_| true), Outcome::Allowed);
    assert_eq!(table.rate(3), ((19, 32), &[2][..], 0));
    assert_eq!(table.rate(positions[&(19, 9)]).1.len(), RATE_LIMIT);
    table.sweep(2, true);
    assert_eq!(table.summaries(), [(19, 1), (2, 1), (3, 1), (0, 1)]);
    empty(&mut table, 2);

    let mut full = keys(MAX_RATE_KEYS);
    full.reverse();
    full[MAX_RATE_KEYS / 2] = (0, 0);
    let dead = full[1];
    let added = key(8_000);
    let mut crowded = setup::<T>(&full, |_| 1);
    let visits = std::cell::RefCell::new(Vec::new());
    let live = |key| {
        visits.borrow_mut().push(key);
        key != dead
    };
    assert_eq!(crowded.admit(added, 2, &live), Outcome::Allowed);
    let expected_visits: Vec<_> = full.iter().copied().filter(|&key| key != (0, 0)).collect();
    assert_eq!(*visits.borrow(), expected_visits);
    let expected: Vec<_> = full.into_iter().filter(|&key| key != dead).chain([added]).collect();
    verify_index(&mut crowded, &expected, 2);
}

fn visit<T: Table>() -> [u128; CASES.len()] {
    let (one, small, full) = (hit::<T>(1), hit::<T>(32), hit::<T>(MAX_RATE_KEYS));
    let (up, down) = (fill::<T>(false), fill::<T>(true));
    let (miss, backoff) = misses::<T>();
    [one, small, full, up, down, miss, backoff, churn::<T>(),
        expiry::<T>(true), half_retain::<T>(), expiry::<T>(false), shutdown::<T>()]
}

pub(super) fn run() {
    let start = Instant::now();
    let cap = Duration::from_secs(100);
    // Make the backend helpers live in this crate without ever calling the stubs.
    black_box(baseline::process_alive as fn(Key) -> bool);
    black_box(candidate::process_alive as fn(Key) -> bool);
    oracle::<baseline::Limiter>();
    oracle::<candidate::Limiter>();
    const ORDERS: [[usize; 4]; 4] = [[0,1,1,0], [1,0,0,1], [1,0,0,1], [0,1,1,0]];
    let mut deltas: [Vec<i128>; CASES.len()] = std::array::from_fn(|_| Vec::with_capacity(16));
    println!("ROW,block,visit,arm,case,ns,operations");
    for block in 0..=16 {
        let order = if block == 0 { &[0usize, 1][..] } else { &ORDERS[(block - 1) % ORDERS.len()] };
        let mut sums = [[0u128; CASES.len()]; 2];
        for (position, &arm) in order.iter().enumerate() {
            assert!(start.elapsed() < cap, "internal probe budget exhausted");
            let values = if arm == 0 { visit::<baseline::Limiter>() } else { visit::<candidate::Limiter>() };
            assert!(start.elapsed() < cap, "internal probe budget exhausted");
            for (i, (&value, &(case, ops))) in values.iter().zip(&CASES).enumerate() {
                assert!(value > 0);
                sums[arm][i] += value;
                println!("ROW,{block},{position},{arm},{case},{value},{ops}");
            }
        }
        if block != 0 {
            for (i, deltas) in deltas.iter_mut().enumerate() {
                deltas.push(sums[1][i] as i128 - sums[0][i] as i128);
            }
        }
    }

    let mut passed = true;
    for (deltas, &(case, ops)) in deltas.iter_mut().zip(&CASES) {
        deltas.sort_unstable();
        let median_numerator = deltas[7] + deltas[8];
        let p90_numerator = deltas[14];
        let case_passed = median_numerator <= 0 && p90_numerator <= 0;
        passed &= case_passed;
        // Deltas are for two visits; an even-sample median averages two deltas.
        println!("CASE,{case},median_num={median_numerator},median_den={},p90_num={p90_numerator},p90_den={},pass={case_passed}", 4 * ops, 2 * ops);
    }
    println!("COST_SCREEN,pass={passed},blocks=16,cases={}", CASES.len());
}
