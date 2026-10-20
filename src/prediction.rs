use crate::{ConfidenceConstant, most_common_value::confidence_value};
use rug::{Float, ops::Pow};

// Reference recurrence: at most 66 iterations. Retain this approximation while
// using MPFR throughout; it is distinct from the publication's ten iterations.
fn log_run_probability(p: &Float, r: usize, n: usize) -> Float {
    let prec = p.prec();
    let q = Float::with_val(prec, 1 - p);
    let a = Float::with_val(prec, p.clone().pow(r as u64) * &q);
    let mut x = Float::with_val(prec, 1);
    for _ in 0..66 {
        let mut next = x.clone().pow((r + 1) as u64);
        next *= &a;
        next += 1;
        if next == x {
            break;
        }
        x = next;
    }
    let mut numerator = Float::with_val(prec, p * &x);
    numerator = -numerator;
    numerator += 1;
    if numerator <= 0 {
        return Float::with_val(prec, f64::NEG_INFINITY);
    }
    let mut denominator = Float::with_val(prec, &x - 1);
    denominator *= r;
    denominator = -denominator;
    denominator += 1;
    denominator *= q;
    numerator.ln_mut();
    numerator -= denominator.ln();
    numerator -= x.ln() * (n + 1);
    numerator
}

pub(crate) fn estimate(
    c: usize,
    n: usize,
    run: usize,
    k: usize,
    prec: u32,
    constant: ConfidenceConstant,
) -> (Float, Float, Option<Float>, Float, Float) {
    let p_global = Float::with_val(prec, Float::with_val(prec, c) / n);
    let mut global_upper = if c == 0 {
        let mut log = Float::with_val(prec, Float::parse("0.01").unwrap()).ln();
        log /= n;
        -log.exp_m1()
    } else {
        let mut correction = Float::with_val(prec, 1 - &p_global);
        correction *= &p_global;
        correction /= n - 1;
        correction.sqrt_mut();
        correction *= confidence_value(constant, prec);
        correction += &p_global;
        correction
    };
    if global_upper > 1 {
        global_upper = Float::with_val(prec, 1);
    }
    let mut upper = Float::with_val(prec, 1);
    upper /= k;
    if global_upper > upper {
        upper = global_upper.clone();
    }
    let target = Float::with_val(prec, Float::parse("0.99").unwrap()).ln();
    let local = if upper < 1 && log_run_probability(&upper, run + 1, n) > target {
        let mut low = upper.clone();
        let mut high = Float::with_val(prec, 1);
        loop {
            let mut mid = Float::with_val(prec, &low + &high);
            mid /= 2;
            if mid == low || mid == high {
                break;
            }
            if log_run_probability(&mid, run + 1, n) > target {
                low = mid;
            } else {
                high = mid;
            }
        }
        upper = high.clone();
        Some(high)
    } else {
        None
    };
    let entropy = if upper == 1 {
        Float::with_val(prec, 0)
    } else {
        -upper.clone().log2()
    };
    (p_global, global_upper, local, upper, entropy)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_bound_matches_independent_decimal_and_precision() {
        // Python Decimal at 100 digits, recurrence and bisection independently evaluated.
        let expected=Float::with_val(256,Float::parse("0.8956749791099103125037474073125374156731849601688069494642294365690074461051917291050670142141728441").unwrap());
        let a = estimate(100, 10000, 20, 2, 256, ConfidenceConstant::NistReference);
        assert!(a.2.is_some());
        assert!(
            (a.4.clone() - expected).abs() < Float::with_val(256, Float::parse("1e-68").unwrap())
        );
        let b = estimate(100, 10000, 20, 2, 512, ConfidenceConstant::NistReference);
        assert!(
            (Float::with_val(512, a.4) - b.4).abs()
                < Float::with_val(512, Float::parse("1e-68").unwrap())
        );
        let p = a.2.unwrap();
        let target = Float::with_val(256, Float::parse("0.99").unwrap()).ln();
        assert!(
            (log_run_probability(&p, 21, 10000) - target).abs()
                < Float::with_val(256, Float::parse("1e-68").unwrap())
        );
    }
    #[test]
    fn long_run_matches_decimal_despite_native_reference_roundoff() {
        // NIST biased-random-bytes bitstring counts, independent Decimal(100) oracle.
        let expected = Float::with_val(256, Float::parse("0.04192513364633918757603426482966537058393141412930567658633904507039563563473520377532080305218014363").unwrap());
        let r = estimate(
            7198538,
            7999937,
            582,
            2,
            256,
            ConfidenceConstant::NistReference,
        );
        assert!((r.4 - expected).abs() < Float::with_val(256, Float::parse("1e-65").unwrap()));
    }
    #[test]
    fn ring_oscillator_local_bound_decimal_oracle() {
        let expected=Float::with_val(256,Float::parse("0.2905192273658884571682512254081624949982748789301635411416732570747506995532987143637783691012425619").unwrap());
        let result = estimate(
            561973,
            999937,
            82,
            2,
            256,
            ConfidenceConstant::NistReference,
        );
        assert!((result.4 - expected).abs() < Float::with_val(256, Float::parse("1e-65").unwrap()));
    }
    #[test]
    fn alphabet_floor_and_global_bound() {
        let a = estimate(0, 10000, 0, 2, 256, ConfidenceConstant::NistReference);
        assert_eq!(a.3, 0.5);
        assert_eq!(a.4, 1);
        assert!(a.2.is_none());
        let b = estimate(100, 100, 100, 2, 256, ConfidenceConstant::NistReference);
        assert_eq!(b.4, 0);
    }
}
