use std::time::Instant;

use equix::EquiXBuilder;
use rand::RngCore;

use nullspace_crypt::hash::Hash;
use nullspace_structs::directory::{PowAlgo, PowSeed, PowSolution};

pub fn solve_pow(seed: &PowSeed) -> anyhow::Result<PowSolution> {
    match seed.algo {
        PowAlgo::EquiX { effort } => solve_equix_pow(seed, effort),
    }
}

fn solve_equix_pow(seed: &PowSeed, effort: u64) -> anyhow::Result<PowSolution> {
    tracing::debug!(effort, "solving an equix PoW...");
    let start = Instant::now();
    let mut nonce = rand::rng().next_u64();
    let eq = EquiXBuilder::new();
    loop {
        let challenge = Hash::keyed_digest(&seed.seed.to_bytes(), &nonce.to_be_bytes());
        let solutions = match eq.solve(&challenge.to_bytes()) {
            Ok(solutions) => solutions,
            Err(_) => {
                nonce = nonce.wrapping_add(1);
                continue;
            }
        };
        for solution in solutions {
            let bytes = solution.to_bytes();
            let sol_hash = Hash::digest(&bytes);
            let mut first = [0u8; 8];
            first.copy_from_slice(&sol_hash.to_bytes()[..8]);
            let value = u64::from_be_bytes(first);
            if value.checked_mul(effort).is_some() {
                tracing::debug!(
                    effort,
                    elapsed = debug(start.elapsed()),
                    "solved an equix PoW!"
                );
                return Ok(PowSolution {
                    seed: seed.seed,
                    nonce,
                    solution: bytes.to_vec().into(),
                });
            }
        }
        nonce = nonce.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use nullspace_structs::timestamp::Timestamp;

    #[test]
    fn solve_pow_returns_valid_solution() {
        let start = Instant::now();
        let seed = PowSeed {
            algo: PowAlgo::EquiX { effort: 100 },
            seed: Hash::from_bytes([7u8; 32]),
            use_before: Timestamp(0),
        };
        let solution = solve_pow(&seed).expect("solve pow");
        assert_eq!(solution.seed, seed.seed);
        assert_eq!(solution.solution.len(), equix::Solution::NUM_BYTES);
        eprintln!("{:?}", start.elapsed())
    }
}
