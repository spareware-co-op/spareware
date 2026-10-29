//! Gate 1 (piece G43): the inputs that tell the correctly rounded ports' mutants apart from the
//! ports themselves.
//!
//! The first full mutation run of the kernel (kernel-quality, every mutant of every crate) left
//! 323 mutants of `src/math/` alive. The mutation judge counts only the integration tests, and the
//! ports' accurate paths, rounding filters, hard-case tables and scaling branches are reached only
//! by hard-to-round or extreme inputs, which the other `math_*` tests sample but did not hit. Each
//! input below was found by running every surviving mutant over CORE-MATH's worst-case lists (the
//! five `.wc` files `kernel-nightly.yml` fetches, commit `0ce0cbc1`, SHA-256 checked), sinpi's
//! set-(b) images, atan2's worst cases scaled exactly by powers of two (atan2 is scale-invariant,
//! so a scaled worst case reaches the subnormal and near-overflow branches), every table index of
//! the reductions, and edge and random inputs, and taking the first input whose result changed.
//! The search harness is not kept here; what it found is.
//!
//! Every input is checked against the rigorous reference of `math_reference.rs` (big-integer
//! interval arithmetic, refined until both ends round alike), never against the port itself, with
//! what each public function owes (`reference::owed`): a value bit for bit, or the error.
//!
//! The mutants that no input can tell apart (unreachable guards, filters made stricter, constants
//! that round alike) are in `quality-ledger.json`'s `survivors`, each with its reason; a few of
//! those reasons rest on a `libm` value, checked at the end of this file.

// Test code may use native arithmetic and casts (Cargo.toml); helpers outside `#[test]` functions
// may panic, as clippy.toml allows inside them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may use native arithmetic, casts and std reductions, and stop loudly"
)]

#[path = "math_reference.rs"]
mod reference;

use geom_num::{NumError, Real, math};
use reference::{Owed, assert_owed, owed};

/// The outcome of a call of a public maths function, to compare with what it owes.
fn outcome(r: Result<Real, NumError>) -> Owed {
    match r {
        Ok(v) => Owed::Value(v.get()),
        Err(NumError::NotFinite) => Owed::NotFinite,
        Err(NumError::OutsideDomain) => Owed::OutsideDomain,
        Err(e) => panic!("a maths function returned {e:?}"),
    }
}

/// Checks a function of one argument on each input (given by its bits) against the reference.
#[track_caller]
fn check_each(
    name: &str,
    inputs: &[u64],
    ours: fn(f64) -> Result<Real, NumError>,
    oracle: fn(f64) -> f64,
) {
    for &bits in inputs {
        let x = f64::from_bits(bits);
        assert_owed(
            outcome(ours(x)),
            owed(&[x], oracle(x)),
            &format!("{name}({x:e} = {bits:#018x})"),
        );
    }
}

/// exp's 21 listed hard cases (exp.rs `HARD`), which no other integration test calls.
#[rustfmt::skip]
const EXP_DATABASE: [u64; 21] = [
    0xc038_f80e_06f3_a04c, 0xc01d_af3a_fee5_1991, 0xbfd0_7e05_e6e0_3b83,
    0xbfbd_af69_3d64_fada, 0xbfba_8f78_3d74_9a8f, 0xbf94_0a99_0279_c2c0,
    0xbf2a_2fef_efd5_80df, 0xbf16_6471_6b68_a409, 0xbe30_401a_e484_09b5,
    0xbe1f_f171_507f_8ba5, 0xbe18_5e60_704a_3a9c, 0xbe04_bd46_601a_e1ef,
    0x3ca0_0000_0000_0000, 0x3dbf_fff7_fffe_0001, 0x3dd8_cd99_fffb_319c,
    0x3e04_13f3_3fe6_ce07, 0x3e1f_8e16_5f83_88f7, 0x3eb2_82aa_e316_9ee7,
    0x3ebe_5ba9_2aa9_b1ef, 0x3f76_a4d1_af9c_c989, 0x3ffa_ca7a_e8da_5a7b,
];

/// exp.wc inputs whose results are subnormal or near the subnormal threshold.
#[rustfmt::skip]
const EXP_SUBNORMAL: [u64; 32] = [
    0xc086_232c_09d1_5037, 0xc086_2bee_d65e_d3f6, 0xc086_34a4_7d9f_5238,
    0xc086_3d94_0f11_7024, 0xc086_466f_6707_3eea, 0xc086_4f4f_4eb2_bf11,
    0xc086_57f8_4ad0_5c05, 0xc086_60d8_b9f5_9055, 0xc086_69c2_1056_c79e,
    0xc086_7272_b3ad_838f, 0xc086_7bce_59f0_23cb, 0xc086_8488_f101_c6b0,
    0xc086_8d43_3c8c_1ff9, 0xc086_95ac_b72d_e899, 0xc086_9e5a_911d_1523,
    0xc086_a6f0_7cb5_1993, 0xc086_afb8_754a_00fb, 0xc086_b858_2123_ad8c,
    0xc086_c129_94ab_2857, 0xc086_ca80_1976_7a2d, 0xc086_d38f_ece7_890e,
    0xc086_dc76_5121_6e80, 0xc086_e542_e201_2b48, 0xc086_edfb_e286_2a00,
    0xc086_f702_df27_85fe, 0xc086_ffc8_84dc_97fc, 0xc087_08b6_c0aa_c348,
    0xc087_118f_39e1_86bf, 0xc087_1a75_2f3b_72c9, 0xc087_2363_b701_9f07,
    0xc087_2c65_c933_0627, 0xc087_35e1_f175_b127,
];

/// exp.wc inputs whose results lie between 2^-1075 and 2^-1074, where the subnormal branch's
/// result has one bit and its test straddles the midpoint 2^-1075.
#[rustfmt::skip]
const EXP_LAST_BIT: [u64; 11] = [
    0xc087_4385_446d_71c2, 0xc087_4385_446d_71c3, 0xc087_4385_446d_71c4,
    0xc087_43cd_b0a9_76ba, 0xc087_46f5_4d9f_be31, 0xc087_488b_75d3_c805,
    0xc087_4910_d52d_304f, 0xc087_4910_d52d_3050, 0xc087_4910_d52d_3051,
    0xc087_4910_d52d_3052, 0xc087_4910_d52d_3053,
];

/// log.wc inputs in the accurate path's table slices i = 307, 315 and 316 (C indices 179, 187,
/// 188).
#[rustfmt::skip]
const LN_TABLE_SLICES: [u64; 12] = [
    0x0001_3378_9cb8_6cd6, 0x1d33_3b3c_4b43_b25d, 0x3cf3_35c4_c2a2_5329,
    0x5c93_3e05_65e0_1e40, 0x0113_b756_15ba_5196, 0x1d23_b72f_bf6c_bdaa,
    0x3d63_b52b_04e0_b5e7, 0x5d93_b6d5_1d6e_05ec, 0x0053_c09c_882f_af0c,
    0x2173_c8e0_6e32_a4ec, 0x3fd3_cf8f_24cf_f6d6, 0x6083_c7c2_0b39_765d,
];

/// log.wc inputs on which a mutant of the accurate path changes the result, found by replaying
/// that path exactly (128-bit integers) over the whole list off CI: subnormal arguments (the
/// significand's normalising shift), one just below 1 where the Taylor terms of degree 13 and
/// 14 matter, and the first ones in the table slices of entries 179, 187 and 188.
#[rustfmt::skip]
const LN_REPLAY: [u64; 11] = [
    0x0000_0b77_51df_aafa, 0x0000_87b5_0e3c_0a7f, 0x0001_104d_89f0_2a00,
    0x0001_1b74_c68c_438e, 0x3fef_e575_5f3a_16e2, 0x0001_3378_9cb8_6cd6,
    0x0093_3c33_6dad_ad4c, 0x0113_b756_15ba_5196, 0x0173_b12c_5de9_2c06,
    0x0053_cda6_e6bd_78d0, 0x0073_cc6b_3500_2c2d,
];

/// The log.wc inputs in [1 - 2^-8, 1) nearest 1 - 2^-8, where the accurate path's Taylor series
/// runs at its largest |z|.
#[rustfmt::skip]
const LN_NEAR_ONE: [u64; 24] = [
    0x3fef_e575_5f3a_16e2, 0x3fef_e734_51b9_c74f, 0x3fef_eaa7_d732_e9e8,
    0x3fef_edae_7b62_22e5, 0x3fef_edf4_7fc2_216d, 0x3fef_f469_58c4_b10e,
    0x3fef_f695_8ffd_9247, 0x3fef_f782_81f6_daf5, 0x3fef_f84c_c30e_14c0,
    0x3fef_f851_252f_37c4, 0x3fef_f854_57d5_b75e, 0x3fef_f89c_a54e_7d96,
    0x3fef_f8a2_8e42_9f33, 0x3fef_f8b9_f732_93a4, 0x3fef_f8d1_b28f_012b,
    0x3fef_f8f8_cdea_67c1, 0x3fef_f916_99c2_2c36, 0x3fef_f929_ea5e_6553,
    0x3fef_f92b_de37_d8b6, 0x3fef_f95a_6cf7_b7c2, 0x3fef_f964_955a_c2a7,
    0x3fef_f991_6b0c_d9cc, 0x3fef_fa03_4539_0cf9, 0x3fef_fa18_c565_80de,
];

/// log.wc's subnormal inputs, which the accurate path reads without the fast path's scaling.
#[rustfmt::skip]
const LN_SUBNORMAL: [u64; 24] = [
    0x0000_0000_0000_0001, 0x0000_3848_c7de_27dd, 0x0000_d864_eea8_ed58,
    0x0001_1b74_c68c_438e, 0x0001_5d46_7ed4_e8c1, 0x0002_10ec_b340_264c,
    0x0002_483e_71c4_8997, 0x0002_9d20_4655_850a, 0x0003_e02b_ce0c_f0f7,
    0x0004_7609_196c_917b, 0x0005_5a71_a1f0_1e0d, 0x0005_7f11_408b_2353,
    0x0006_1dcf_2ed7_23c4, 0x0006_7134_9642_929e, 0x0006_a02a_0542_c9f8,
    0x0007_2131_e05b_2645, 0x0008_9ed0_aa7c_1064, 0x0009_288b_2a77_0d5d,
    0x000a_bae6_73b6_1d1d, 0x000b_22ef_71f7_1bc7, 0x000b_a405_b37a_e36b,
    0x000c_95fb_f91e_8b91, 0x000e_8b46_38a3_c84c, 0x000e_e0f1_d6e3_d717,
];

/// sinpi.wc inputs on the general path (0x1.2p-5 < |x| < 2^41).
#[rustfmt::skip]
const SINPI_GENERAL: [u64; 32] = [
    0x3fa2_08ee_c90c_a0f4, 0x3fa9_9411_4a4f_606c, 0x3fb0_e4cf_2ba8_3c93,
    0x3fb7_bce2_25ef_e541, 0x3fbe_1ba1_7d53_4d89, 0x3fc3_dbe2_3752_5567,
    0x3fc9_437b_a05a_35ac, 0x3fce_b628_7a28_fa0b, 0x3fd4_49bd_d02d_1ad6,
    0x3fd9_ebda_48a0_d200, 0x3fdf_91d3_70aa_5bec, 0x3fe3_6901_3a17_726e,
    0x3fe7_2658_45f9_fe90, 0x3fea_b8de_e816_8d6b, 0x3fee_6c30_e016_d752,
    0x4005_9334_7495_7eb6, 0x4017_c5f0_7147_f8d6, 0x4026_3a6c_5a21_c0a1,
    0x4032_cb79_20ce_a8bc, 0x403e_9850_96dd_7cb8, 0x404c_b5f7_bd7d_9c3e,
    0x4057_6618_4d08_ceaf, 0x4061_26cc_dca7_40aa, 0x406b_0d11_db3b_43e3,
    0x4077_a366_6e53_a055, 0xbfab_d8a1_e8a3_e82c, 0xbfbc_1b30_d457_c36d,
    0xbfc8_3c84_5fa5_ca54, 0xbfd3_6642_2fd2_e52a, 0xbfde_8e42_2fd2_e52a,
    0xbfe4_db21_17e9_7295, 0xbfea_6f21_17e9_7295,
];

/// sinpi.wc inputs and set-(b) images on the near-zero paths (|x| <= 0x1.2p-5).
#[rustfmt::skip]
const SINPI_NEAR_ZERO: [u64; 32] = [
    0x3ca0_003f_49d0_e4d1, 0x3cd0_28b4_de50_eb16, 0x3d00_5ea8_03f1_5684,
    0x3d30_afde_120d_401e, 0x3d61_b01c_7d73_1343, 0x3d90_ff61_dc86_7aff,
    0x3dc1_49e3_3669_d2b7, 0x3df0_4e8d_22bb_9b1b, 0x3e20_e6b6_f6fe_eb23,
    0x3e52_7443_e99d_029e, 0x3e81_8d6b_19a5_48df, 0x3eb2_b81e_017e_789a,
    0x3ee2_7cd8_6303_1a4a, 0x3f14_8b67_53e8_7e71, 0x3f43_ad9e_b364_dcb0,
    0x3f72_60de_830c_aacb, 0x3850_0000_0000_0000, 0x3854_00b2_a0a5_dbdc,
    0x3857_f6c6_18d0_f10f, 0x385b_fadb_4d77_e1ab, 0x3c10_0000_0000_0000,
    0x3c14_00b2_a0a5_dbdc, 0x3c17_f6c6_18d0_f10f, 0x3c1b_fadb_4d77_e1ab,
    0x3e90_0000_0000_0000, 0x3e94_00b2_a0a5_dbdc, 0x3e97_f6c6_18d0_f10f,
    0x3e9b_fadb_4d77_e1ab, 0x3f30_0000_0000_0000, 0x3f34_00b2_a0a5_dbdc,
    0x3f37_f6c6_18d0_f10f, 0x3f3b_fadb_4d77_e1ab,
];

/// The sinpi.wc inputs on the near-zero paths (2^-54 <= |x| <= 0x1.2p-5) whose
/// sine lies closest to a midpoint (52 to 57 identical bits after the round bit).
#[rustfmt::skip]
const SINPI_NEAR_ZERO_HARDEST: [u64; 24] = [
    0x3cdf_339a_b577_31d3, 0x3dda_d53f_8db9_2af0, 0x3daa_8851_b86f_2c07,
    0x3e6c_6b06_98c2_8d70, 0x3ea5_f3aa_a98d_64bb, 0x3d5a_65da_b603_e8b2,
    0x3f52_872c_d25f_2925, 0x3f57_31f6_c052_0517, 0x3db5_2f22_4a74_f956,
    0x3eec_215e_afd4_639d, 0x3f1b_98f3_bf70_e3b6, 0x3f75_a568_87be_3f23,
    0x3e31_e234_5136_0f11, 0x3dd6_fe50_9010_e3c6, 0x3f34_0e17_3f72_a36d,
    0x3e86_c9f2_9f59_9930, 0x3f95_f230_fa0b_27d8, 0x3e77_fe3a_2303_8269,
    0x3d20_9a60_6e18_8bc7, 0x3f63_b057_8ec6_eba8, 0x3deb_0bc9_cc64_068f,
    0x3e8a_4be2_2136_3f8d, 0x3f23_2550_f04b_da91, 0x3e1b_c8ef_6c81_59e0,
];

/// The cospi.wc inputs on the near-zero path (2^-29 < |x| <= 2^-12) whose
/// cosine lies closest to a midpoint (52 to 59 identical bits after the round bit).
#[rustfmt::skip]
const COSPI_NEAR_ZERO_HARDEST: [u64; 24] = [
    0x3e78_2428_46e3_d0af, 0x3ebf_9a45_ce01_134f, 0x3e38_f346_b4c3_05bf,
    0x3e99_d9ce_0957_f1c8, 0x3e71_92cf_274b_2b70, 0x3ed7_0c3e_3f71_bd49,
    0x3ee5_7252_95c9_52eb, 0x3ed9_cbff_3284_a4fb, 0x3e7a_e77c_c45f_8687,
    0x3e2c_cf64_29be_6622, 0x3eb5_95ab_6556_fa30, 0x3e43_0e5c_ded6_7caf,
    0x3efb_0180_20fc_41c3, 0x3e54_b00c_b56d_bc65, 0x3edf_d7f6_ce68_f482,
    0x3ef8_e05e_bd2d_ad28, 0x3e45_9b8b_1f4e_cc99, 0x3ed1_f23a_5c93_5fbb,
    0x3ef9_5ad0_9666_9072, 0x3ece_31da_2e76_0ba6, 0x3efe_b98e_6c83_b9ba,
    0x3e49_f816_7cdf_5123, 0x3f14_c79b_f81f_84ae, 0x3ec3_51e4_f633_4ad4,
];

/// Near-zero hard cases on which `sticky`'s nudge of the low part decides the rounding (sinpi's
/// and cospi's near-zero accurate paths), found by replaying those paths exactly over every
/// listed near-zero input of sinpi.wc (with set (b)'s images) and cospi.wc, off CI.
#[rustfmt::skip]
const SINPI_STICKY: [u64; 5] = [
    0x3d0e_cced_b62e_c785, 0x3d5a_65da_b603_e8b2, 0x3cdf_339a_b577_31d3,
    0xbdda_d53f_8db9_2af0, 0x3dd6_fe50_9010_e3c6,
];

/// See [`SINPI_STICKY`]. The last one is a general-path cospi input that `refine`'s hard-case
/// lookup must not match by its table index alone (found by replaying `refine` the same way).
#[rustfmt::skip]
const COSPI_STICKY: [u64; 5] = [
    0x3e43_0e5c_ded6_7caf, 0xbe99_d9ce_0957_f1c8, 0x3e78_2428_46e3_d0af, 0xbebf_9a45_ce01_134f,
    0x3f65_bbef_584e_580d,
];

/// cospi.wc inputs on the general path (2^-12 < |x| < 2^41).
#[rustfmt::skip]
const COSPI_GENERAL: [u64; 32] = [
    0x3f30_01f3_d2f6_438c, 0x3f3d_94a5_726a_91c9, 0x3f4a_723d_c3a9_57ca,
    0x3f57_6737_e28a_e63c, 0x3f64_7109_c86b_7c5a, 0x3f71_3585_69fd_055d,
    0x3f7d_fb9c_9094_4f02, 0x3f89_8fcc_c46d_4978, 0x3f94_b819_9dc9_5b44,
    0x3f9f_6287_f6bf_0777, 0x3fa8_fd6e_33ee_fcd3, 0x3fb2_0d0c_ba22_3566,
    0x3fb9_f65a_aad8_cfb2, 0x3fc1_937b_a05a_35ac, 0x3fc8_73d5_db96_bbb4,
    0x3fcf_4391_d540_67ff, 0x3fd5_c806_f0d7_2384, 0x3fdc_9aa3_f67c_1ffb,
    0x3fe2_0caf_c67f_e293, 0x3fe6_7721_17e9_7295, 0x3fea_e37e_78b9_cd77,
    0x3fef_7721_17e9_7295, 0x3ffb_36f5_d233_27cf, 0x4007_8d60_5b57_283c,
    0x4013_bc53_1643_3ed9, 0x4020_47e0_2dba_d047, 0x402c_bb68_939f_d4cd,
    0x403a_b486_df31_5744, 0x4049_f447_6ced_0f8c, 0x4057_12de_4833_aa2f,
    0x4062_a000_0000_0000, 0x406e_a2ee_24c4_bc1d,
];

/// cospi.wc inputs on the near-zero path (2^-29 < |x| <= 2^-12).
#[rustfmt::skip]
const COSPI_NEAR_ZERO: [u64; 24] = [
    0x3e2c_cf64_29be_6622, 0x3e34_5f30_6dc9_c8c8, 0x3e41_a47c_7ee5_a4e6,
    0x3e4b_e531_1f33_57c5, 0x3e56_c6c2_3dcf_d363, 0x3e61_a47c_7ee5_a514,
    0x3e6c_69db_6085_f828, 0x3e77_407b_158e_4322, 0x3e81_ebf6_0864_6ac1,
    0x3e8c_6c98_56fa_3ad6, 0x3e97_5e3f_3284_9c74, 0x3ea1_9229_d4bf_c34b,
    0x3eac_1cff_3238_a9f8, 0x3eb7_1391_6be9_0bc0, 0x3ec3_2921_289d_00b1,
    0x3ece_6b2d_92e6_7392, 0x3ed8_c6ab_6abe_1742, 0x3ee4_5c93_a6c8_3b53,
    0x3eee_7f81_d617_48a5, 0x3ef9_b237_3a8a_b16f, 0x3f04_4cda_4206_95f0,
    0x3f0f_4197_bdb1_556f, 0x3f1a_2fc4_3e6e_11f6, 0x3f25_3335_582b_aa89,
];

/// atan2.wc pairs scaled exactly by a power of two (atan2 is scale-invariant) until both are
/// subnormal.
#[rustfmt::skip]
const ATAN2_SUBNORMAL: [(u64, u64); 21] = [
    (0x0000_0001_0000_0000, 0x0000_1000_0000_0000), (0x0000_097b_3b67_96fa, 0x0000_1011_827b_f8b6),
    (0x0000_0bfd_8d22_895c, 0x0000_12dd_cf24_a57c), (0x0000_0d9c_19d8_b816, 0x0000_173a_2657_378b),
    (0x0000_1000_0000_0000, 0x0000_0001_0000_0000), (0x0000_1000_0000_0000, 0x0000_0800_0000_0000),
    (0x0000_1000_0000_0000, 0x0000_1000_0000_0000), (0x0000_1000_0000_0000, 0x8000_1000_0000_0000),
    (0x0000_13a6_b6ef_ff79, 0x0000_17b0_b576_52f8), (0x0000_144f_1618_3c78, 0x0000_0e19_8d2c_6dbe),
    (0x0000_1556_18d3_7dda, 0x0000_1711_8719_198e), (0x0000_1641_2021_2b50, 0x0000_1ca2_0d97_1e5a),
    (0x8000_1000_0000_0000, 0x0000_1000_0000_0000), (0x8000_1000_0000_0000, 0x8000_1000_0000_0000),
    (0x0000_0000_0000_1000, 0x0000_0000_0100_0000), (0x0000_0000_0100_0000, 0x0000_0000_0000_1000),
    (0x0000_0000_0100_0000, 0x0000_0000_0080_0000), (0x0000_0000_0100_0000, 0x0000_0000_0100_0000),
    (0x0000_0000_0100_0000, 0x8000_0000_0100_0000), (0x8000_0000_0100_0000, 0x0000_0000_0100_0000),
    (0x8000_0000_0100_0000, 0x8000_0000_0100_0000),
];

/// atan2.wc pairs scaled exactly into the top binade (the fast path halves x > 2^1022).
#[rustfmt::skip]
const ATAN2_HUGE: [(u64, u64); 24] = [
    (0x0000_0000_0000_0001, 0x7fef_ffff_ffff_ffff), (0x7fd6_16eb_1964_9cd3, 0x7fe1_e247_2167_73c1),
    (0x7fd9_3c2a_37d3_530f, 0x7fe4_9069_4fa8_769c), (0x7fdb_80cb_b84f_081c, 0x7fe1_913e_cbe1_6f25),
    (0x7fdd_456b_d865_cc21, 0x7fe2_e897_315e_40c3), (0x7fde_ec2e_c862_5f65, 0x7feb_17e1_2ca4_f5f0),
    (0x7fe0_2bc2_6477_8f4a, 0x7fda_bcce_4e0e_2265), (0x7fe0_cb77_7d85_9f7d, 0x7fea_893f_e0e1_6d49),
    (0x7fe1_680f_afa6_8494, 0x7fee_e4da_d5b7_936d), (0x7fe2_0420_e019_13f2, 0x7fea_5f94_e93e_e84e),
    (0x7fe2_b1a7_e7be_7511, 0x7fd9_3c2f_b760_6d82), (0x7fe3_6068_1f3d_b8dd, 0x7fea_fa78_aabd_eede),
    (0x7fe4_14e8_d6b0_5f5e, 0x7fdd_fcee_b3ff_477c), (0x7fe4_d406_086a_0bbb, 0x7fdc_b072_2425_588c),
    (0x7fe5_8f13_7cd3_3d05, 0x7fe5_5b81_d9a3_937f), (0x7fe6_61b1_55f7_8708, 0x7fe7_83a7_f660_e9f3),
    (0x7fe7_363b_fc91_a888, 0x7fed_2683_c7b8_972c), (0x7fe8_1f1f_2139_8b05, 0x7fef_1bf9_1ece_d9f8),
    (0x7fe9_0d6e_f45e_b405, 0x7fe8_507d_e0f4_4e3a), (0x7fea_1526_4a3a_9682, 0x7fe8_94fd_899f_5bbb),
    (0x7feb_2f5c_8582_69cc, 0x7fec_6b6b_5c90_bc59), (0x7fec_61c2_4b1c_c09c, 0x7fea_e19d_564c_6adf),
    (0x7fed_b488_80fc_6e71, 0x7fed_46e6_656a_d311), (0x7fef_17c5_1ae9_698c, 0x7fea_5e5c_9f5f_5e5c),
];

/// atan2.wc pairs scaled exactly below 2^-920 while normal (the fast path's scaling up).
#[rustfmt::skip]
const ATAN2_TINY: [(u64, u64); 24] = [
    (0x010b_6bd7_6d8f_3c63, 0x8490_82cd_f45b_895f), (0x0486_f550_9939_7db7, 0x0494_027f_8e90_01d5),
    (0x0489_b947_d13f_07d9, 0x0490_3409_f29c_8397), (0x048b_d64f_d72b_2d59, 0x0490_39cd_e60a_f50a),
    (0x048d_87f9_d419_72f4, 0x049a_885d_fc36_902e), (0x048f_24b4_c976_95fd, 0x0491_e5cd_9a96_4dc4),
    (0x0490_3f08_60f0_a97f, 0x0296_dd2f_b2b9_444e), (0x0490_dbd7_6912_0d1a, 0x0494_1289_51d8_b679),
    (0x0491_7487_8aa6_086a, 0x0446_1138_948b_7892), (0x0492_0d86_76a1_533f, 0x0487_f326_580b_a57e),
    (0x0492_b6f0_8edc_2f68, 0x048f_eeb9_f670_bbe6), (0x0493_6313_217c_91a9, 0x048e_5b6c_2370_03d1),
    (0x0494_1352_6529_97c2, 0x0495_6d15_8c26_98f1), (0x0494_cf05_664b_e374, 0x0491_b732_4657_3eb0),
    (0x0495_8645_66b5_bd8d, 0x0493_2bab_4caf_2829), (0x0496_5118_6041_7f8d, 0x049a_64e3_c1c7_fcd3),
    (0x0497_21de_8c48_5038, 0x049e_a141_895a_666e), (0x0498_0055_62d8_a331, 0x029d_1354_59ff_ae53),
    (0x0498_e7e1_244f_c556, 0x049e_309a_3079_584f), (0x0499_e9fc_2bd3_dbaa, 0x0496_840b_829b_6d17),
    (0x049a_fc65_527a_c188, 0x049e_0198_6538_cff9), (0x049c_2505_d3bc_12ee, 0x02b2_fb63_5397_3059),
    (0x049d_6911_deb5_e3bb, 0x0497_0c94_4733_6067), (0x049e_bc95_b0cd_d3ec, 0x049a_0774_75c6_6c20),
];

/// atan2.wc pairs whose divisor is a power of two (x, or y when |y| > |x|): exact reciprocals in
/// the accurate path.
#[rustfmt::skip]
const ATAN2_POW2: [(u64, u64); 48] = [
    (0x0000_0000_0000_0001, 0x0000_0000_0000_0001), (0x033c_334c_56a3_18a9, 0x86c0_0000_0000_0000),
    (0x0834_4434_617e_7dd8, 0x49e0_0000_0000_0000), (0x0d7f_ffff_ffff_fffe, 0x4f40_0000_0000_0000),
    (0x138a_a29e_7ffd_27e7, 0x5380_0000_0000_0000), (0x19df_ffff_ffff_ffff, 0x59e0_0000_0000_0000),
    (0x1edf_ffff_ffff_ffff, 0x60a0_0000_0000_0000), (0x25c0_0000_0000_0001, 0x6820_0000_0000_0000),
    (0x2abf_ffff_ffff_ffff, 0x6ca0_0000_0000_0000), (0x305f_ffff_ffff_ffff, 0x7100_0000_0000_0000),
    (0x3515_ce74_4895_3c33, 0x7640_0000_0000_0000), (0x39c7_fa2c_f7c2_6d13, 0x7c40_0000_0000_0000),
    (0x7fef_ffff_ffff_ffff, 0x8000_0000_0000_0001), (0x82e2_f2e7_8dcd_06c5, 0x4300_0000_0000_0000),
    (0x8820_0000_0000_0001, 0x4ae0_0000_0000_0000), (0x8d5f_ffff_ffff_ffff, 0x90c0_0000_0000_0000),
    (0x9360_0000_0000_0001, 0x5660_0000_0000_0000), (0x9960_0000_0000_0001, 0x5c60_0000_0000_0000),
    (0x9ec0_0000_0000_0001, 0x60e0_0000_0000_0000), (0xa4a8_f4ac_8aec_a820, 0x6620_0000_0000_0000),
    (0xaa7e_9d87_9126_031a, 0x6ac0_0000_0000_0000), (0xaffd_3bf7_be9f_31a9, 0x7240_0000_0000_0000),
    (0xb4c0_0000_0000_0001, 0x7520_0000_0000_0000), (0xb948_99b3_fa3d_a987, 0x7a80_0000_0000_0000),
    (0x0010_0000_0000_0000, 0x0000_0000_0000_0001), (0x0010_0000_0000_0000, 0x000f_ffff_ffff_ffff),
    (0x0010_0000_0000_0000, 0x8000_0000_0000_0001), (0x0010_0000_0000_0000, 0x800f_ffff_ffff_ffff),
    (0x03a0_0000_0000_0000, 0x8390_d367_326d_b020), (0x3ff0_0000_0000_0000, 0x0000_0000_0000_0001),
    (0x3ff0_0000_0000_0000, 0x000f_ffff_ffff_ffff), (0x3ff0_0000_0000_0000, 0x0010_0000_0000_0000),
    (0x3ff0_0000_0000_0000, 0x0172_3456_7890_0000), (0x3ff0_0000_0000_0000, 0x3f30_0000_0000_0000),
    (0x3ff0_0000_0000_0000, 0x3fe0_0000_0000_0000), (0x3ff0_0000_0000_0000, 0x8000_0000_0000_0001),
    (0x3ff0_0000_0000_0000, 0x800f_ffff_ffff_ffff), (0x3ff0_0000_0000_0000, 0x8010_0000_0000_0000),
    (0x7fe0_0000_0000_0000, 0x3ff0_0000_0000_0000), (0x8010_0000_0000_0000, 0x0000_0000_0000_0001),
    (0x8010_0000_0000_0000, 0x8000_0000_0000_0001), (0x8010_0000_0000_0000, 0x800f_ffff_ffff_ffff),
    (0x83a0_0000_0000_0000, 0x8390_d367_326d_b020), (0xbff0_0000_0000_0000, 0x0000_0000_0000_0001),
    (0xbff0_0000_0000_0000, 0x000f_ffff_ffff_ffff), (0xbff0_0000_0000_0000, 0x0010_0000_0000_0000),
    (0xbff0_0000_0000_0000, 0x8000_0000_0000_0001), (0xbff0_0000_0000_0000, 0x800f_ffff_ffff_ffff),
];

/// atan2.wc pairs with |y| > |x| (the accurate path's pi/2 - z) and with x < 0 (its pi -+ z).
#[rustfmt::skip]
const ATAN2_OCTANTS: [(u64, u64); 48] = [
    (0x0002_e92a_716f_90df, 0x0002_9cef_331b_53f7), (0x4322_8eed_3b57_b57c, 0x4319_154d_92b2_575c),
    (0x432b_6d43_a23c_b544, 0x4325_1a1d_b661_2e24), (0x4331_5ad8_a305_0452, 0x432d_6b81_aeef_3254),
    (0x4334_3657_cd33_ad04, 0x4331_8353_5717_eb37), (0x4336_9b39_91cd_1690, 0x4330_1f26_8c8a_1175),
    (0x4338_bd0c_563a_60ca, 0x4330_8e6f_fb38_34a1), (0x433a_b3f3_047b_224a, 0x4338_560f_66b2_996b),
    (0x433c_90e8_83db_b96a, 0x433a_ccb3_ae6a_19e5), (0x433e_22d8_4acf_1314, 0x4337_83ad_952b_51df),
    (0x433f_f463_fab4_e51c, 0x433e_756d_e668_43e3), (0x4340_d6f8_a2ec_ebdb, 0x4337_aee8_8bba_1a99),
    (0x4341_b216_794d_4717, 0x433f_ea38_45ae_5a4e), (0x4342_bcd8_b82c_dd67, 0x4340_fbce_768c_7291),
    (0x4343_dc07_e467_0b30, 0x433e_db15_9ce2_3b5f), (0x4345_14b5_1161_60f5, 0x433e_c4c1_40dc_a20e),
    (0x4346_6332_c3d2_fce6, 0x433f_3618_3f35_2767), (0x4347_e34d_aab8_54ea, 0x433f_a241_6ffb_cb1f),
    (0x4349_c807_cfac_7263, 0x4345_1e87_3ee5_9899), (0x434b_c6fe_665b_76ff, 0x4346_976c_587f_a32b),
    (0x434d_ea26_cd54_f959, 0x4347_f2f9_69dd_9c4c), (0x434f_ffa5_b07e_f925, 0x434a_012d_8d7b_cca3),
    (0x4399_32a8_03b7_219a, 0x433c_9635_fb02_b529), (0x4420_b502_a958_5cff, 0x433c_94ad_723d_7b78),
    (0x0000_0000_0000_0001, 0x8000_0000_0000_0001), (0x000f_ffff_ffff_ffff, 0xffef_ffff_ffff_ffff),
    (0x01c0_0000_0000_0000, 0x853f_ffff_ffff_fffe), (0x04b4_3aef_6f31_fa85, 0x8803_9916_8be7_89e6),
    (0x0900_0000_0000_0000, 0x8c6b_6b0b_ed24_4b0b), (0x0c52_d005_5755_0c3f, 0x8fc0_0000_0000_0001),
    (0x115f_ffff_ffff_fffe, 0x94b1_99ca_f58a_e24b), (0x2239_6067_bb25_2798, 0xa590_5686_763f_944c),
    (0x3ff0_0000_0000_0000, 0x800f_ffff_ffff_ffff), (0x477e_b011_9ff0_50fc, 0xcad7_1e6b_23a2_9dc6),
    (0x58c6_f4d8_2517_5afc, 0xdc1e_17cc_9239_7869), (0x6b5f_ffff_ffff_ffff, 0xeeb7_2104_c2bb_f824),
    (0x7fef_ffff_ffff_ffff, 0xffef_ffff_ffff_ffff), (0x800f_ffff_ffff_ffff, 0xbff0_0000_0000_0000),
    (0x8155_4da8_cb64_2c53, 0x84d0_b357_725b_e553), (0x849f_ffff_ffff_ffff, 0x87ff_ffff_ffff_fffe),
    (0x8900_0000_0000_0000, 0x8c6b_6b0b_ed24_4b0b), (0x8c52_d005_5755_0c3f, 0x8fc0_0000_0000_0001),
    (0x950e_d528_7156_aa88, 0x985f_ffff_ffff_fffe), (0xa86b_79fb_df90_ed42, 0xabc0_ec00_f857_bfb4),
    (0xbff0_0000_0000_0000, 0x800f_ffff_ffff_ffff), (0xc77e_b011_9ff0_50fc, 0xcad7_1e6b_23a2_9dc6),
    (0xd8c6_f4d8_2517_5afc, 0xdc1e_17cc_9239_7869), (0xeb5f_ffff_ffff_ffff, 0xeeb7_2104_c2bb_f824),
];

/// Pairs built for `accurate`'s exact-quotient shortcut (`atan2.rs`): y subnormal, q = y/x near
/// 2^-33 just above a midpoint of the doubles with atan(q) just below it, so the correctly
/// rounded angle is the double below while t = RN(q) is the one above, and t·x - y is below
/// 2^-1075, so `fma(t, x, -y)` rounds to 0 although t·x != y. Only the residual scaled by 2^105
/// shows that the quotient is inexact. Built with exact rationals off CI.
#[rustfmt::skip]
const ATAN2_FALSE_ZERO: [(u64, u64); 3] = [
    (0x000a_bebe_05f0_ee77, 0x020e_eeba_a42d_d5b7),
    (0x0007_1286_c178_ce6c, 0x0214_2bf2_2193_bcf9),
    (0x0001_99da_0bd8_ce65, 0x01ed_afe3_3d36_d233),
];

/// Checks `atan2` on each pair (given by its bits) in all four sign combinations, and swapped
/// when `swap` (the other octants), against the reference.
#[track_caller]
fn check_atan2(pairs: &[(u64, u64)], swap: bool) {
    for &(yb, xb) in pairs {
        let (y0, x0) = (f64::from_bits(yb), f64::from_bits(xb));
        let mut cases = vec![(y0, x0)];
        if swap {
            cases.push((x0, y0));
        }
        for (y, x) in cases {
            for (y, x) in [(y, x), (-y, x), (y, -x), (-y, -x)] {
                assert_owed(
                    outcome(math::atan2(y, x)),
                    owed(&[y, x], reference::atan2(y, x)),
                    &format!("atan2({:#018x}, {:#018x})", y.to_bits(), x.to_bits()),
                );
            }
        }
    }
}

/// `exp`'s accurate path at its listed hard cases, where it looks the result up (the lookup's
/// test on the low part's significand, and its table), and at hard cases with subnormal results.
#[test]
fn exp_hard_cases_and_subnormal_results() {
    check_each("exp", &EXP_DATABASE, math::exp, reference::exp);
    check_each("exp", &EXP_SUBNORMAL, math::exp, reference::exp);
    check_each("exp", &EXP_LAST_BIT, math::exp, reference::exp);
}

/// `ln`'s accurate path: three slices of its 128-bit tables no other test reaches, the Taylor
/// series just below 1 at its largest argument, and subnormal arguments, which the accurate path
/// reads from the input itself rather than from the fast path's scaled copy.
#[test]
fn ln_accurate_path_tables_taylor_and_subnormals() {
    check_each("ln", &LN_TABLE_SLICES, math::ln, reference::ln);
    check_each("ln", &LN_NEAR_ONE, math::ln, reference::ln);
    check_each("ln", &LN_SUBNORMAL, math::ln, reference::ln);
    check_each("ln", &LN_REPLAY, math::ln, reference::ln);
}

/// `sinpi` and `cospi` on hard cases of their general and near-zero paths, both signs: the fast
/// paths' rounding tests decide close to a midpoint, and the accurate paths round what they
/// leave.
#[test]
fn sinpi_and_cospi_hard_cases_both_signs() {
    let sinpi = |x: f64| reference::sinpi(x) + 0.0;
    let cospi = |x: f64| reference::cospi(x) + 0.0;
    for list in [
        &SINPI_GENERAL[..],
        &SINPI_NEAR_ZERO[..],
        &SINPI_NEAR_ZERO_HARDEST[..],
        &SINPI_STICKY[..],
    ] {
        let both: Vec<u64> = list.iter().flat_map(|&b| [b, b ^ (1 << 63)]).collect();
        check_each("sinpi", &both, math::sinpi, sinpi);
    }
    for list in [
        &COSPI_GENERAL[..],
        &COSPI_NEAR_ZERO[..],
        &COSPI_NEAR_ZERO_HARDEST[..],
        &COSPI_STICKY[..],
    ] {
        let both: Vec<u64> = list.iter().flat_map(|&b| [b, b ^ (1 << 63)]).collect();
        check_each("cospi", &both, math::cospi, cospi);
    }
}

/// `atan2` on hard cases moved by an exact power of two into the scaled branches: both
/// arguments subnormal (the accurate path's reciprocal of a subnormal), the top binade (the fast
/// path's halving) and below 2^-920 (its scaling up); each also swapped, for the other octants.
#[test]
fn atan2_scaled_hard_cases() {
    check_atan2(&ATAN2_SUBNORMAL, true);
    check_atan2(&ATAN2_HUGE, true);
    check_atan2(&ATAN2_TINY, true);
}

/// `atan2` on hard cases whose divisor is a power of two (an exact reciprocal in the accurate
/// path's 192-bit division), with |y| > |x| (its pi/2 - z) and with x < 0 (its pi -+ z).
#[test]
fn atan2_hard_cases_in_every_octant() {
    check_atan2(&ATAN2_POW2, false);
    check_atan2(&ATAN2_OCTANTS, false);
}

/// `accurate`'s exact-quotient shortcut must not trust `fma(t, x, -y) == 0` for a tiny y, where
/// an inexact residual underflows to 0: these pairs need the scaled check (or the full
/// evaluation), and their angle is the double below t.
#[test]
fn atan2_tiny_quotients_whose_residual_underflows() {
    check_atan2(&ATAN2_FALSE_ZERO, false);
}

/// The unproven `sinpi` and `cospi` special-case the quarter turns; the ledger keeps three of their
/// mutants as equivalent because `libm` already gives the same exact value there: `sin(−π̃/2)` and
/// `cos(±π̃)` (with `π̃ = RN(π)`) round to exactly −1, since `π − π̃ ≈ 1.2·10^−16` moves the cosine
/// from ±1 by about 10^−32. This pins those `libm` values, so a `libm` change that broke them would
/// show here and not only as a changed result of the unproven functions.
#[test]
fn libm_gives_the_quarter_turns_exactly() {
    let ok = |r: Result<Real, NumError>| r.unwrap().get();
    assert_eq!(ok(math::unproven::sin(-core::f64::consts::FRAC_PI_2)), -1.0);
    assert_eq!(ok(math::unproven::cos(core::f64::consts::PI)), -1.0);
    assert_eq!(ok(math::unproven::cos(-core::f64::consts::PI)), -1.0);
    // The unproven functions at those quarter turns, through their own special cases.
    assert_eq!(ok(math::unproven::sinpi(-0.5)), -1.0);
    assert_eq!(ok(math::unproven::sinpi(1.5)), -1.0);
    assert_eq!(ok(math::unproven::cospi(1.0)), -1.0);
    assert_eq!(ok(math::unproven::cospi(-1.0)), -1.0);
    assert_eq!(ok(math::unproven::cospi(3.0)), -1.0);
}
