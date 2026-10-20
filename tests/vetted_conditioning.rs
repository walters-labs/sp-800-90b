use rug::Float;
use sp_800_90b::*;

#[test]
fn table_one_hash_families() {
    use ConditioningHash::*;
    for (hash, width) in [
        (Sha1, 160),
        (Sha224, 224),
        (Sha256, 256),
        (Sha384, 384),
        (Sha512, 512),
        (Sha512_224, 224),
        (Sha512_256, 256),
        (Sha3_224, 224),
        (Sha3_256, 256),
        (Sha3_384, 384),
        (Sha3_512, 512),
    ] {
        for component in [
            VettedConditioner::Hash(hash),
            VettedConditioner::Hmac(hash),
            VettedConditioner::HashDf(hash),
        ] {
            assert_eq!(component.output_bits(), width);
            assert_eq!(component.internal_width(), width);
            assert!(component.validate_dimensions(width, width).is_ok());
            assert!(component.validate_dimensions(width / 2, width).is_err());
            assert!(component.validate_dimensions(width, width * 2).is_err());
        }
    }
}

#[test]
fn aes_block_size_is_distinct_from_key_size() {
    for bits in [128, 192, 256] {
        let key = ConditioningAesKey::try_from(bits).unwrap();
        assert_eq!(key.bits(), bits);
        for mac in [VettedConditioner::Cmac(key), VettedConditioner::CbcMac(key)] {
            assert_eq!((mac.output_bits(), mac.internal_width()), (128, 128));
        }
        let df = VettedConditioner::BlockCipherDf(key);
        assert_eq!((df.output_bits(), df.internal_width()), (bits, bits));
    }
    for bits in [0, 64, 129, 160, 512, u32::MAX] {
        assert_eq!(
            ConditioningAesKey::try_from(bits),
            Err(ConditioningError::InvalidAesKeySize { bits })
        );
    }
}

#[test]
fn typed_assessment_retains_metadata_and_truncation() {
    let component = VettedConditioner::Hash(ConditioningHash::Sha512_256);
    let entropy = Float::with_val(256, 400);
    let options = ConditioningOptions {
        retained_output_bits: Some(128),
        ..Default::default()
    };
    let typed = component.assess(512, &entropy, options).unwrap();
    let manual =
        conditioning_entropy(512, 256, 256, &entropy, ConditioningKind::Vetted, options).unwrap();
    assert_eq!(typed.vetted_component, Some(component));
    assert_eq!(manual.vetted_component, None);
    assert_eq!(typed.min_entropy, manual.min_entropy);
    assert_eq!(
        typed.untruncated_min_entropy,
        manual.untruncated_min_entropy
    );
    assert_eq!(typed.retained_output_bits, 128);
    let short = component
        .assess(32, &Float::with_val(256, 16), Default::default())
        .unwrap();
    assert_eq!(short.effective_internal_width, 32);
    assert_eq!(short.output_bits, 256);
    assert_eq!(short.vetted_component, Some(component));
    assert!(
        component
            .assess(
                512,
                &entropy,
                ConditioningOptions {
                    retained_output_bits: Some(257),
                    ..Default::default()
                }
            )
            .is_err()
    );
}
