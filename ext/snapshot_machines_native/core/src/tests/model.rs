//! Randomized runs against an independent naive model.

use matryona_machines::testing::SplitMix64;
use std::vec::Vec;

use super::*;

/// Independent naive model: a growable chain, linear search, the whole
/// history of attempts and how each one ended.
/// A model chain entry as the manifest the catalog reports.
fn manifest(m: (u64, Option<u64>, u64)) -> Manifest {
    Manifest::new(id(m.0), m.1.map(id), m.2).expect("the model links its chain")
}

struct Model {
    scope: u16,
    capacity: usize,
    issued: u64,
    chain: Vec<(u64, Option<u64>, u64)>,
    /// Every attempt ever begun: (id, base), in order.
    attempts: Vec<(u64, Option<u64>)>,
    open: bool,
    /// Whether the latest attempt ended in an abort.
    aborted: bool,
}

impl Model {
    /// Scope in the top 16 bits, sequence below, spelled out here rather
    /// than borrowed from the crate under test.
    fn raw(&self, sequence: u64) -> u64 {
        (u64::from(self.scope) << 48) | sequence
    }

    fn owns(&self, raw: u64) -> bool {
        raw >> 48 == u64::from(self.scope) && (1..=self.issued).contains(&(raw & 0xFFFF_FFFF_FFFF))
    }

    fn begin(&mut self, base: Option<u64>) -> Result<(u64, Option<u64>), Refusal> {
        if base.is_some_and(|base| !self.owns(base)) {
            return Err(Refusal::Stale);
        }
        if self.open {
            return Err(Refusal::Busy);
        }
        if base.is_some() && self.chain.last().map(|m| m.0) != base {
            return Err(Refusal::NotLatest);
        }
        let used = if base.is_some() { self.chain.len() } else { 0 };
        if used >= self.capacity {
            return Err(Refusal::ChainFull);
        }
        self.issued += 1;
        let attempt = (self.raw(self.issued), base);
        self.attempts.push(attempt);
        self.open = true;
        self.aborted = false;
        Ok(attempt)
    }

    fn commit(
        &mut self,
        attempt: (u64, Option<u64>),
        checksum: u64,
    ) -> Result<Committing, Refusal> {
        if !self.owns(attempt.0) {
            return Err(Refusal::Stale);
        }
        let latest = self.attempts.last().copied();
        if self.open && latest.map(|a| a.0) == Some(attempt.0) {
            if latest != Some(attempt) {
                return Err(Refusal::Mismatch);
            }
            if attempt.1.is_none() {
                self.chain.clear();
            }
            self.chain.push((attempt.0, attempt.1, checksum));
            self.open = false;
            return Ok(Committing::Committed(manifest((
                attempt.0, attempt.1, checksum,
            ))));
        }
        match self.chain.iter().find(|m| m.0 == attempt.0) {
            Some(m) if m.1 == attempt.1 && m.2 == checksum => {
                Ok(Committing::AlreadyCommitted(manifest(*m)))
            }
            Some(_) => Err(Refusal::Mismatch),
            None => Err(Refusal::Stale),
        }
    }

    fn abort(&mut self, attempt: (u64, Option<u64>)) -> Result<Aborting, Refusal> {
        if !self.owns(attempt.0) {
            return Err(Refusal::Stale);
        }
        let latest = self.attempts.last().copied();
        if (self.open || self.aborted) && latest.map(|a| a.0) == Some(attempt.0) {
            if latest != Some(attempt) {
                return Err(Refusal::Mismatch);
            }
            if self.open {
                self.open = false;
                self.aborted = true;
                return Ok(Aborting::Aborted);
            }
            return Ok(Aborting::AlreadyAborted);
        }
        Err(Refusal::Stale)
    }

    /// A restart: the chain and the sequence survive, the attempt does not.
    fn restart(&mut self) {
        self.open = false;
        self.aborted = false;
    }

    fn plan(&self, target: u64, room: usize) -> Result<Vec<Manifest>, Refusal> {
        if !self.owns(target) {
            return Err(Refusal::Stale);
        }
        let at = self
            .chain
            .iter()
            .position(|m| m.0 == target)
            .ok_or(Refusal::Stale)?;
        if room <= at {
            return Err(Refusal::Undersized);
        }
        Ok(self.chain[..=at].iter().copied().map(manifest).collect())
    }
}

/// A draw in `0..len`.
fn index(draws: &mut SplitMix64, len: usize) -> usize {
    usize::try_from(draws.below(len as u64)).expect("below a usize bound")
}

/// An id to feed the catalog: one it issued, the next one, or one of
/// another scope with the same sequence.
fn some_id(draws: &mut SplitMix64, model: &Model) -> u64 {
    let sequence = 1 + draws.below(model.issued + 1);
    if draws.below(5) == 0 {
        let other = model
            .scope
            .wrapping_add(1 + u16::try_from(draws.below(3)).expect("below three"));
        return (u64::from(other) << 48) | sequence;
    }
    model.raw(sequence)
}

/// An attempt to hand back: one really begun, or one rebuilt with another
/// base or another scope.
fn some_attempt(draws: &mut SplitMix64, model: &Model) -> (u64, Option<u64>) {
    if !model.attempts.is_empty() && draws.below(4) != 0 {
        let pick = model.attempts[index(draws, model.attempts.len())];
        if draws.below(6) != 0 {
            return pick;
        }
    }
    let raw = some_id(draws, model);
    let base = (draws.below(2) == 0 && raw & 0xFFFF_FFFF_FFFF > 1).then(|| raw - 1);
    (raw, base)
}

/// The model's attempt as a `Pending`; `None` when its parts do not link.
fn rebuilt(attempt: (u64, Option<u64>)) -> Option<Pending> {
    Pending::new(id(attempt.0), attempt.1.map(id))
}

fn agree<const N: usize>(catalog: &Catalog<N>, model: &Model) {
    assert_eq!(catalog.issued(), model.issued);
    assert_eq!(catalog.latest(), model.chain.last().copied().map(manifest));
    let open = model.attempts.last().filter(|_| model.open);
    assert_eq!(
        catalog
            .pending()
            .map(|p| (p.id().raw(), p.base().map(SnapshotId::raw))),
        open.copied()
    );
}

fn run_against_model<const N: usize>(scope: u16, seed: u64, steps: usize) {
    let mut draws = SplitMix64::new(seed);
    let mut catalog = Catalog::<N>::with_scope(scope);
    let mut model = Model {
        scope,
        capacity: N,
        issued: 0,
        chain: Vec::new(),
        attempts: Vec::new(),
        open: false,
        aborted: false,
    };
    let (mut refusals, mut repeats) = (0_usize, 0_usize);
    for _ in 0..steps {
        let before = catalog.clone();
        let checksum = draws.below(3);
        let (refused, repeated) = match draws.below(9) {
            0..=2 => {
                let base = match draws.below(4) {
                    0 => None,
                    1 => model.chain.last().map(|m| m.0),
                    _ => Some(some_id(&mut draws, &model)),
                };
                let got = catalog.begin(base.map(id));
                let want = model.begin(base);
                assert_eq!(
                    got.map(|p| (p.id().raw(), p.base().map(SnapshotId::raw))),
                    want
                );
                (got.is_err(), false)
            }
            3..=5 => {
                let attempt = some_attempt(&mut draws, &model);
                let Some(pending) = rebuilt(attempt) else {
                    continue;
                };
                let got = catalog.commit(pending, checksum);
                assert_eq!(got, model.commit(attempt, checksum));
                (
                    got.is_err(),
                    matches!(got, Ok(Committing::AlreadyCommitted(_))),
                )
            }
            6 => {
                let attempt = some_attempt(&mut draws, &model);
                let Some(pending) = rebuilt(attempt) else {
                    continue;
                };
                let got = catalog.abort(pending);
                assert_eq!(got, model.abort(attempt));
                (got.is_err(), got == Ok(Aborting::AlreadyAborted))
            }
            7 => {
                let target = some_id(&mut draws, &model);
                let room = index(&mut draws, N + 2);
                let mut out = [Manifest::default(); 8];
                let got = catalog.restore_plan(id(target), &mut out[..room]);
                let want = model.plan(target, room);
                assert_eq!(got.map(|n| out[..n].to_vec()), want);
                if got.is_err() {
                    assert_eq!(out, [Manifest::default(); 8], "a refused plan wrote");
                }
                (got.is_err(), false)
            }
            8 => {
                let saved: Vec<Manifest> = model.chain.iter().copied().map(manifest).collect();
                catalog = Catalog::restored(scope, catalog.issued(), &saved)
                    .expect("a catalog's own saved state restores");
                model.restart();
                (false, false)
            }
            other => panic!("draw {other} is outside the operations"),
        };
        if refused {
            refusals += 1;
            assert_eq!(catalog, before, "a refused call changed the catalog");
        }
        if repeated {
            repeats += 1;
            assert_eq!(catalog, before, "an honoured repeat changed the catalog");
        }
        agree(&catalog, &model);
    }
    assert!(refusals > 0, "the run never exercised a refusal");
    assert!(repeats > 0, "the run never exercised an honoured repeat");
}

#[test]
fn randomized_operations_match_the_naive_model() {
    for seed in 0..24 {
        let scope = [0, 7, u16::MAX][usize::try_from(seed % 3).expect("below three")];
        run_against_model::<3>(scope, seed, 1_500);
    }
    for seed in 100..112 {
        run_against_model::<1>(0, seed, 800);
        run_against_model::<6>(513, seed, 1_500);
    }
}
