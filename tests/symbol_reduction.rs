use sp_800_90b::*;

#[test]
fn ranking_order_and_sample_boundaries() {
    let mut ranking = vec![12, 0, 9, 3];
    ranking.extend((0..16).filter(|p| ![12, 0, 9, 3].contains(p)));
    let mapping = SymbolReduction::new(16, 4, &ranking).unwrap();
    assert_eq!(
        mapping.reduce(&[0x1000, 1, 0x200, 8, 0xffff, 0]).unwrap(),
        [8, 4, 2, 1, 15, 0]
    );
    assert_eq!(mapping.ranking(), ranking);
    assert_eq!(mapping.input_bits(), 16);
    assert_eq!(mapping.output_bits(), 4);
    assert_eq!(mapping.entropy_ceiling(), 4);
    assert!(mapping.reduce(&[]).unwrap().is_empty());
}

#[test]
fn exhaustive_sixteen_bit_projection_matches_independent_bitstring_oracle() {
    let ranking: Vec<u8> = (0..16).rev().collect();
    let mapping = SymbolReduction::new(16, 8, &ranking).unwrap();
    let samples: Vec<u128> = (0..=65535).collect();
    let reduced = mapping.reduce(&samples).unwrap();
    let mut counts = [0usize; 256];
    for (&sample, &symbol) in samples.iter().zip(&reduced) {
        let bits = format!("{sample:016b}");
        assert_eq!(symbol, u8::from_str_radix(&bits[..8], 2).unwrap());
        counts[usize::from(symbol)] += 1;
    }
    // Uniform 16-bit inputs collapse to 256 equally populated buckets:
    // at most eight bits per original sample survive the mapping.
    assert!(counts.iter().all(|&count| count == 256));
}

#[test]
fn low_bit_example_and_extreme_widths() {
    // Rank low bits in descending significance to preserve the low nibble.
    let mut ranking = vec![3, 2, 1, 0];
    ranking.extend(4..32);
    let mapping = SymbolReduction::new(32, 4, &ranking).unwrap();
    assert_eq!(
        mapping.reduce(&[0x1234_abcd, 0xffff_fff2]).unwrap(),
        [13, 2]
    );
    let full = SymbolReduction::new(128, 8, &(0..128).rev().collect::<Vec<_>>()).unwrap();
    assert_eq!(
        full.reduce(&[u128::MAX, 1 << 127, 1, 0]).unwrap(),
        [255, 128, 0, 0]
    );
    let bit = SymbolReduction::new(1, 1, &[0]).unwrap();
    assert_eq!(bit.reduce(&[0, 1]).unwrap(), [0, 1]);
    assert!(bit.reduce(&[2]).is_err());
}

#[test]
fn invalid_configuration_and_samples() {
    use SymbolReductionError::*;
    for width in [0, 129, 255] {
        assert!(matches!(
            SymbolReduction::new(width, 1, &[]),
            Err(InvalidInputWidth { .. })
        ));
    }
    for width in [0, 5, 9] {
        assert!(matches!(
            SymbolReduction::new(4, width, &[0, 1, 2, 3]),
            Err(InvalidOutputWidth { .. })
        ));
    }
    assert!(matches!(
        SymbolReduction::new(4, 2, &[0, 1]),
        Err(RankingLength { .. })
    ));
    assert!(matches!(
        SymbolReduction::new(4, 2, &[0, 1, 2, 4]),
        Err(PositionOutOfRange {
            rank: 4,
            position: 4
        })
    ));
    // Validate the discarded portion of the ranking too.
    assert!(matches!(
        SymbolReduction::new(4, 2, &[0, 1, 2, 2]),
        Err(DuplicatePosition { position: 2 })
    ));
    let mapping = SymbolReduction::new(16, 4, &(0..16).collect::<Vec<_>>()).unwrap();
    assert_eq!(
        mapping.reduce(&[0, 1 << 16]),
        Err(SampleOutOfRange {
            index: 1,
            value: 1 << 16,
            bits: 16
        })
    );
}
