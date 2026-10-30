//! Reed-Solomon over GF(2^8) (primitive polynomial 0x11d, first root alpha^0)
//! with errors-and-erasures decoding: Forney syndromes, Berlekamp-Massey,
//! Chien search and the Forney algorithm. A codeword is the data followed by
//! `nsym` parity bytes and corrects `e` errors plus `E` erasures when
//! `2e + E <= nsym`. Polynomials are stored highest degree first.

use std::sync::OnceLock;

struct Tables {
    exp: [u8; 512],
    log: [u8; 256],
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut t = Tables { exp: [0; 512], log: [0; 256] };
        let mut x: u16 = 1;
        for i in 0..255 {
            t.exp[i] = x as u8;
            t.log[x as usize] = i as u8;
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= 0x11d;
            }
        }
        for i in 255..512 {
            t.exp[i] = t.exp[i - 255];
        }
        t
    })
}

fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let t = tables();
    t.exp[t.log[a as usize] as usize + t.log[b as usize] as usize]
}

fn div(a: u8, b: u8) -> u8 {
    debug_assert!(b != 0);
    if a == 0 {
        return 0;
    }
    let t = tables();
    t.exp[(t.log[a as usize] as usize + 255 - t.log[b as usize] as usize) % 255]
}

fn inverse(a: u8) -> u8 {
    div(1, a)
}

/// alpha^i for any integer i.
fn alpha(i: i64) -> u8 {
    tables().exp[i.rem_euclid(255) as usize]
}

fn poly_scale(p: &[u8], x: u8) -> Vec<u8> {
    p.iter().map(|&c| mul(c, x)).collect()
}

fn poly_add(p: &[u8], q: &[u8]) -> Vec<u8> {
    let len = p.len().max(q.len());
    let mut r = vec![0; len];
    for (i, &c) in p.iter().enumerate() {
        r[i + len - p.len()] = c;
    }
    for (i, &c) in q.iter().enumerate() {
        r[i + len - q.len()] ^= c;
    }
    r
}

fn poly_mul(p: &[u8], q: &[u8]) -> Vec<u8> {
    let mut r = vec![0; p.len() + q.len() - 1];
    for (j, &b) in q.iter().enumerate() {
        for (i, &a) in p.iter().enumerate() {
            r[i + j] ^= mul(a, b);
        }
    }
    r
}

fn poly_eval(p: &[u8], x: u8) -> u8 {
    p.iter().skip(1).fold(p[0], |y, &c| mul(y, x) ^ c)
}

fn generator(nsym: usize) -> Vec<u8> {
    (0..nsym).fold(vec![1], |g, i| poly_mul(&g, &[1, alpha(i as i64)]))
}

/// Return `data` followed by `nsym` parity bytes.
pub(crate) fn encode(data: &[u8], nsym: usize) -> Vec<u8> {
    assert!(data.len() + nsym <= 255, "codeword longer than 255 bytes");
    let g = generator(nsym);
    let mut out = data.to_vec();
    out.resize(data.len() + nsym, 0);
    for i in 0..data.len() {
        let coef = out[i];
        if coef != 0 {
            for (j, &gj) in g.iter().enumerate().skip(1) {
                out[i + j] ^= mul(gj, coef);
            }
        }
    }
    out[..data.len()].copy_from_slice(data);
    out
}

/// Syndromes with a leading zero, as the Forney evaluator expects.
fn syndromes(msg: &[u8], nsym: usize) -> Vec<u8> {
    std::iter::once(0).chain((0..nsym).map(|i| poly_eval(msg, alpha(i as i64)))).collect()
}

fn forney_syndromes(synd: &[u8], erasures: &[usize], n: usize) -> Vec<u8> {
    let mut fsynd = synd[1..].to_vec();
    for &p in erasures {
        let x = alpha((n - 1 - p) as i64);
        for j in 0..fsynd.len().saturating_sub(1) {
            fsynd[j] = mul(fsynd[j], x) ^ fsynd[j + 1];
        }
    }
    fsynd
}

/// Berlekamp-Massey on Forney syndromes: the locator of the unknown errors.
fn error_locator(fsynd: &[u8], nsym: usize, erase_count: usize) -> Option<Vec<u8>> {
    let mut err_loc = vec![1u8];
    let mut old_loc = vec![1u8];
    for k in 0..nsym - erase_count {
        let mut delta = fsynd[k];
        for j in 1..err_loc.len() {
            delta ^= mul(err_loc[err_loc.len() - 1 - j], fsynd[k.checked_sub(j)?]);
        }
        old_loc.push(0);
        if delta != 0 {
            if old_loc.len() > err_loc.len() {
                let new_loc = poly_scale(&old_loc, delta);
                old_loc = poly_scale(&err_loc, inverse(delta));
                err_loc = new_loc;
            }
            err_loc = poly_add(&err_loc, &poly_scale(&old_loc, delta));
        }
    }
    let first = err_loc.iter().position(|&c| c != 0)?;
    let err_loc = err_loc[first..].to_vec();
    let errors = err_loc.len() - 1;
    if 2 * errors + erase_count > nsym {
        return None;
    }
    Some(err_loc)
}

/// Chien search: message positions where the locator has a root.
fn find_errors(err_loc_rev: &[u8], n: usize) -> Option<Vec<usize>> {
    let positions: Vec<usize> = (0..n).filter(|&i| poly_eval(err_loc_rev, alpha(i as i64)) == 0).map(|i| n - 1 - i).collect();
    (positions.len() == err_loc_rev.len() - 1).then_some(positions)
}

/// Forney algorithm: fix the bytes at `positions` given the syndromes.
fn correct_errata(msg: &mut [u8], synd: &[u8], positions: &[usize]) -> Option<()> {
    let n = msg.len();
    let coef_pos: Vec<usize> = positions.iter().map(|&p| n - 1 - p).collect();
    let err_loc = coef_pos.iter().fold(vec![1u8], |loc, &cp| poly_mul(&loc, &poly_add(&[1], &[alpha(cp as i64), 0])));
    // Evaluator: (reversed syndromes * locator) mod x^(len(locator)), then reversed.
    let synd_rev: Vec<u8> = synd.iter().rev().copied().collect();
    let product = poly_mul(&synd_rev, &err_loc);
    let mut err_eval = product[product.len() - err_loc.len()..].to_vec();
    err_eval.reverse();
    let eval_rev: Vec<u8> = err_eval.iter().rev().copied().collect();

    let x: Vec<u8> = coef_pos.iter().map(|&cp| alpha(cp as i64)).collect();
    for (i, &xi) in x.iter().enumerate() {
        let xi_inv = inverse(xi);
        let loc_prime = x.iter().enumerate().filter(|&(j, _)| j != i).fold(1u8, |acc, (_, &xj)| mul(acc, 1 ^ mul(xi_inv, xj)));
        if loc_prime == 0 {
            return None;
        }
        let y = mul(xi, poly_eval(&eval_rev, xi_inv));
        msg[positions[i]] ^= div(y, loc_prime);
    }
    Some(())
}

/// Repair a codeword in place. `erasures` are positions known to be bad.
/// Returns `None` when the damage is beyond what `nsym` parity can repair
/// (detected, not guaranteed: callers must also check integrity).
pub(crate) fn correct(codeword: &mut [u8], nsym: usize, erasures: &[usize]) -> Option<()> {
    let n = codeword.len();
    if erasures.len() > nsym || n > 255 || n <= nsym || erasures.iter().any(|&p| p >= n) {
        return None;
    }
    for &p in erasures {
        codeword[p] = 0;
    }
    let synd = syndromes(codeword, nsym);
    if synd.iter().all(|&s| s == 0) {
        return Some(());
    }
    let fsynd = forney_syndromes(&synd, erasures, n);
    let err_loc = error_locator(&fsynd, nsym, erasures.len())?;
    let err_loc_rev: Vec<u8> = err_loc.iter().rev().copied().collect();
    let errors = find_errors(&err_loc_rev, n)?;
    let mut positions = erasures.to_vec();
    positions.extend(errors.iter().filter(|p| !erasures.contains(p)));
    let mut fixed = codeword.to_vec();
    correct_errata(&mut fixed, &synd, &positions)?;
    if syndromes(&fixed, nsym).iter().any(|&s| s != 0) {
        return None;
    }
    codeword.copy_from_slice(&fixed);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small deterministic generator so failures reproduce.
    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    fn distinct_positions(rng: &mut XorShift, n: usize, count: usize) -> Vec<usize> {
        let mut out = Vec::new();
        while out.len() < count {
            let p = rng.below(n);
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn clean_codeword_has_zero_syndromes() {
        let cw = encode(b"hello handoff", 10);
        assert!(syndromes(&cw, 10).iter().all(|&s| s == 0));
    }

    #[test]
    fn repairs_any_mix_within_budget() {
        let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
        for round in 0..4000 {
            let nsym = 2 + rng.below(60);
            let k = 1 + rng.below(255 - nsym);
            let data: Vec<u8> = (0..k).map(|_| rng.next() as u8).collect();
            let clean = encode(&data, nsym);
            let n = clean.len();
            let erasures = rng.below(nsym + 1);
            let errors = rng.below((nsym - erasures) / 2 + 1);
            let spots = distinct_positions(&mut rng, n, (erasures + errors).min(n));
            let mut damaged = clean.clone();
            for &p in &spots {
                damaged[p] ^= 1 + rng.below(255) as u8;
            }
            let erased: Vec<usize> = spots.iter().take(erasures).copied().collect();
            assert_eq!(correct(&mut damaged, nsym, &erased), Some(()), "round {round}: n={n} nsym={nsym} E={erasures} e={errors}");
            assert_eq!(damaged, clean, "round {round}");
        }
    }

    #[test]
    fn beyond_budget_never_returns_unchecked_garbage() {
        // Past the budget the decoder may refuse or land on another valid
        // codeword; it must never return bytes that fail the syndrome check.
        let mut rng = XorShift(42);
        for _ in 0..2000 {
            let nsym = 8;
            let data: Vec<u8> = (0..40).map(|_| rng.next() as u8).collect();
            let mut cw = encode(&data, nsym);
            let count = 5 + rng.below(10);
            for p in distinct_positions(&mut rng, cw.len(), count) {
                cw[p] ^= 1 + rng.below(255) as u8;
            }
            if correct(&mut cw, nsym, &[]).is_some() {
                assert!(syndromes(&cw, nsym).iter().all(|&s| s == 0));
            }
        }
    }
}
