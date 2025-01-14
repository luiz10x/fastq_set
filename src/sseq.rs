// Copyright (c) 2018 10x Genomics, Inc. All rights reserved.

//! Sized, stack-allocated container for a short DNA sequence.

use crate::array::{ArrayContent, ByteArray};
use std::iter::Iterator;
use std::str;

const UPPER_ACGTN: &[u8; 5] = b"ACGTN";
const N_BASE_INDEX: usize = 4;

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct SSeqContents;

impl ArrayContent for SSeqContents {
    /// Make sure that the input byte slice contains only
    /// "ACGTN" characters. Panics otherwise with an error
    /// message describing the position of the first character
    /// that is not an ACGTN.
    fn validate_bytes(seq: &[u8]) {
        for (i, &s) in seq.iter().enumerate() {
            if !UPPER_ACGTN.iter().any(|&c| c == s) {
                panic!("Non ACGTN character {} at position {}", s, i);
            };
        }
    }
    fn expected_contents() -> &'static str {
        "An [ACGTN]* string"
    }
}

/// Fixed-sized container for a short DNA sequence, with capacity determined by type `N`.
/// Used as a convenient container for barcode or UMI sequences.
/// An `SSeqGen` is guaranteed to contain only "ACGTN" alphabets
pub type SSeqGen<const N: usize> = ByteArray<SSeqContents, N>;

/// Fixed-sized container for a short DNA sequence, up to 23bp in length.
/// Used as a convenient container for barcode or UMI sequences.
/// An `SSeq` is guaranteed to contain only "ACGTN" alphabets
pub type SSeq = SSeqGen<23>;

impl<const N: usize> SSeqGen<N> {
    /// Returns a byte slice of this sequence's contents.
    /// A synonym for as_bytes().
    pub fn seq(&self) -> &[u8] {
        self.as_bytes()
    }

    /// Returns a byte slice of this sequence's contents.
    /// A synonym for as_bytes().
    pub fn seq_mut(&mut self) -> &mut [u8] {
        self.as_mut_bytes()
    }

    /// Returns true if this sequence contains an N.
    pub fn has_n(&self) -> bool {
        self.iter().any(|&c| c == b'N' || c == b'n')
    }

    /// Returns true if this sequence is a homopolymer.
    pub fn is_homopolymer(&self) -> bool {
        assert!(!self.is_empty());
        self.iter().all(|&c| c == self.seq()[0])
    }

    /// Returns true if the last n characters of this sequence are the specified homopolymer.
    pub fn has_homopolymer_suffix(&self, c: u8, n: usize) -> bool {
        self.len() as usize >= n && self.iter().rev().take(n).all(|&x| x == c)
    }

    /// Returns true if the last n characters of this sequence are T.
    pub fn has_polyt_suffix(&self, n: usize) -> bool {
        self.has_homopolymer_suffix(b'T', n)
    }

    /// Returns a 2-bit encoding of this sequence.
    pub fn encode_2bit_u32(self) -> u32 {
        let mut res: u32 = 0;
        assert!(self.len() <= 16);

        let seq = self.seq();
        for (bit_pos, str_pos) in (0..self.len()).rev().enumerate() {
            let byte: u32 = match seq[str_pos as usize] {
                b'A' => 0,
                b'C' => 1,
                b'G' => 2,
                b'T' => 3,
                _ => panic!("non-ACGT sequence"),
            };

            let v = byte << (bit_pos * 2);

            res |= v;
        }

        res
    }

    pub fn one_hamming_iter(self, opt: HammingIterOpt) -> SSeqOneHammingIter<N> {
        SSeqOneHammingIter::new(self, opt)
    }
}

#[derive(Copy, Clone)]
pub enum HammingIterOpt {
    MutateNBase,
    SkipNBase,
}

/// An iterator over sequences which are one hamming distance away
/// from an `SSeq`. `SSeq` is guaranteed to contain "ACGTN" alphabets.
/// Positions containing "N" or "n" are mutated or skipped
/// depending on the `HammingIterOpt`
pub struct SSeqOneHammingIter<const N: usize> {
    source: SSeqGen<N>,      // Original SSeq from which we need to generate values
    chars: &'static [u8; 5], // Whether it's ACGTN or acgtn
    position: usize,         // Index into SSeq where last base was mutated
    chars_index: usize,      // The last base which was used
    skip_n: bool,            // Whether to skip N bases or mutate them
}

impl<const N: usize> SSeqOneHammingIter<N> {
    fn new(sseq: SSeqGen<N>, opt: HammingIterOpt) -> Self {
        let chars = UPPER_ACGTN;
        SSeqOneHammingIter {
            source: sseq,
            chars,
            position: 0,
            chars_index: 0,
            skip_n: match opt {
                HammingIterOpt::SkipNBase => true,
                HammingIterOpt::MutateNBase => false,
            },
        }
    }
}

impl<const N: usize> Iterator for SSeqOneHammingIter<N> {
    type Item = SSeqGen<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.source.len() {
            return None;
        }
        let base_at_pos = self.source[self.position];
        if (self.skip_n && base_at_pos == self.chars[N_BASE_INDEX])
            || (self.chars_index >= N_BASE_INDEX)
        {
            // this is an "N" or we went through the ACGT bases already at this position
            self.position += 1;
            self.chars_index = 0;
            self.next()
        } else if base_at_pos == self.chars[self.chars_index] {
            self.chars_index += 1;
            self.next()
        } else {
            let mut next_sseq = self.source;
            next_sseq[self.position] = self.chars[self.chars_index];
            self.chars_index += 1;
            Some(next_sseq)
        }
    }
}

#[cfg(test)]
mod sseq_test {
    use super::*;
    use bincode;
    use itertools::{assert_equal, Itertools};
    use proptest::collection::vec;
    use proptest::{prop_assert_eq, proptest};
    use std::collections::HashSet;

    #[test]
    fn sort_test() {
        let s1: &[u8] = b"ACNGTA";
        let s2 = b"TAGTCGGC";
        let s3 = b"CATC";
        let s4 = b"TGTG";
        let s5 = b"";
        let s6 = b"A";
        let s7 = b"AACCATAGCCGGNATC";
        let s8 = b"GAACNAGNTGGA";

        let mut seqs = vec![s1, s2, s3, s4, s5, s6, s7, s8];
        let mut sseqs: Vec<SSeq> = seqs.iter().map(|x| SSeq::from_bytes(x)).collect();

        seqs.sort();
        sseqs.sort();

        for i in 0..seqs.len() {
            assert_eq!(seqs[i], sseqs[i].seq());
        }
    }

    proptest! {
        #[test]
        fn prop_test_sort_sseq(
            ref seqs_str in vec("[ACGTN]{0, 23}", 0usize..=10usize),
        ) {
            let mut seqs = seqs_str.iter().map(|s| s.clone().into_bytes()).collect_vec();
            let mut sseqs: Vec<SSeq> = seqs.iter().map(|x| SSeq::from_bytes(x)).collect();

            seqs.sort();
            sseqs.sort();

            for i in 0..seqs.len() {
                assert_eq!(seqs[i], sseqs[i].seq());
            }
        }
    }

    #[test]
    fn dna_encode() {
        let s1 = SSeq::from_bytes(b"AAAAA");
        assert_eq!(s1.encode_2bit_u32(), 0);

        let s1 = SSeq::from_bytes(b"AAAAT");
        assert_eq!(s1.encode_2bit_u32(), 3);

        let s1 = SSeq::from_bytes(b"AAACA");
        assert_eq!(s1.encode_2bit_u32(), 4);

        let s1 = SSeq::from_bytes(b"AACAA");
        assert_eq!(s1.encode_2bit_u32(), 16);

        let s1 = SSeq::from_bytes(b"AATA");
        assert_eq!(s1.encode_2bit_u32(), 12);
    }

    #[test]
    fn test_serde() {
        let seq = b"AGCTAGTCAGTCAGTA";
        let mut sseqs = Vec::new();
        for _ in 0..4 {
            let s = SSeq::from_bytes(seq);
            sseqs.push(s);
        }

        let mut buf = Vec::new();
        bincode::serialize_into(&mut buf, &sseqs).unwrap();
        let roundtrip: Vec<SSeq> = bincode::deserialize_from(&buf[..]).unwrap();
        assert_eq!(sseqs, roundtrip);
    }

    #[test]
    fn test_serde_json() {
        let seq = SSeq::from_bytes(b"AGCTAGTCAGTCAGTA");
        let json_str = serde_json::to_string(&seq).unwrap();
        assert_eq!(json_str, r#""AGCTAGTCAGTCAGTA""#);
    }

    proptest! {
        #[test]
        fn prop_test_serde_sseq(
            ref seq in "[ACGTN]{0, 23}",
        ) {
            let target = SSeq::from_bytes(seq.as_bytes());
            let encoded: Vec<u8> = bincode::serialize(&target).unwrap();
            let decoded: SSeq = bincode::deserialize(&encoded[..]).unwrap();
            prop_assert_eq!(target, decoded);
        }
        #[test]
        fn prop_test_serde_json_sseq(
            ref seq in "[ACGTN]{0, 23}",
        ) {
            let target = SSeq::from_bytes(seq.as_bytes());
            let encoded = serde_json::to_string_pretty(&target).unwrap();
            let decoded: SSeq = serde_json::from_str(&encoded).unwrap();
            prop_assert_eq!(target, decoded);
        }

        #[test]
        fn prop_test_sseq_push(
            ref seq1 in "[ACGTN]{0, 23}",
            ref seq2 in "[ACGTN]{0, 23}",
        ) {
            if seq1.len() + seq2.len() <= 23 {
                let mut s = SSeq::new();
                s.push(seq1.as_bytes());
                s.push(seq2.as_bytes());
                assert_eq!(s, SSeq::from_iter(seq1.as_bytes().iter().chain(seq2.as_bytes().iter())));
            }
        }
    }

    fn test_hamming_helper(seq: &String, opt: HammingIterOpt, n: u8) {
        let sseq = SSeq::from_bytes(seq.as_bytes());
        // Make sure that the hamming distance is 1 for all elements
        for neighbor in sseq.one_hamming_iter(opt) {
            assert_eq!(
                sseq.seq()
                    .iter()
                    .zip_eq(neighbor.seq().iter())
                    .filter(|(a, b)| a != b)
                    .count(),
                1
            );
        }
        // Make sure that the total number of elements is equal to what we expect.
        let non_n = sseq.seq().iter().filter(|&&s| s != n).count();
        let n_bases = sseq.len() - non_n;
        assert_eq!(
            sseq.one_hamming_iter(opt).collect::<HashSet<_>>().len(),
            match opt {
                HammingIterOpt::SkipNBase => 3 * non_n,
                HammingIterOpt::MutateNBase => 3 * non_n + 4 * n_bases,
            }
        );
    }

    proptest! {
        #[test]
        fn prop_test_one_hamming_upper(
            seq in "[ACGTN]{0, 23}", // 0 and 23 are inclusive bounds
        ) {
            test_hamming_helper(&seq, HammingIterOpt::SkipNBase, b'N');
            test_hamming_helper(&seq, HammingIterOpt::MutateNBase, b'N');
        }
    }

    #[test]
    #[should_panic]
    fn test_sseq_invalid_1() {
        let _ = SSeq::from_bytes(b"ASDF");
    }

    #[test]
    #[should_panic]
    fn test_sseq_invalid_2() {
        let _ = SSeq::from_bytes(b"ag");
    }

    #[test]
    #[should_panic]
    fn test_sseq_too_long() {
        let _ = SSeq::from_bytes(b"GGGACCGTCGGTAAAGCTACAGTGAGGGATGTAGTGATGC");
    }

    #[test]
    fn test_as_bytes() {
        assert_eq!(SSeq::from_bytes(b"ACGT").as_bytes(), b"ACGT");
    }

    #[test]
    fn test_has_n() {
        assert!(SSeq::from_bytes(b"ACGTN").has_n());
        assert!(!SSeq::from_bytes(b"ACGT").has_n());
    }

    #[test]
    fn test_is_homopolymer() {
        assert!(SSeq::from_bytes(b"AAAA").is_homopolymer());
        assert!(!SSeq::from_bytes(b"ACGT").is_homopolymer());
    }

    #[test]
    fn test_has_homopolymer_suffix() {
        assert!(SSeq::from_bytes(b"ACGTAAAAA").has_homopolymer_suffix(b'A', 5));
        assert!(!SSeq::from_bytes(b"ACGTTAAAA").has_homopolymer_suffix(b'A', 5));
        assert!(SSeq::from_bytes(b"CCCCC").has_homopolymer_suffix(b'C', 5));
        assert!(!SSeq::from_bytes(b"GGGG").has_homopolymer_suffix(b'G', 5));
    }

    #[test]
    fn test_has_polyt_suffix() {
        assert!(SSeq::from_bytes(b"CGCGTTTTT").has_polyt_suffix(5));
        assert!(!SSeq::from_bytes(b"CGCGAAAAA").has_polyt_suffix(5));
    }

    #[test]
    fn test_one_hamming_simple() {
        assert_equal(
            SSeq::from_bytes(b"GAT")
                .one_hamming_iter(HammingIterOpt::SkipNBase)
                .collect_vec(),
            vec![
                b"AAT", b"CAT", b"TAT", b"GCT", b"GGT", b"GTT", b"GAA", b"GAC", b"GAG",
            ]
            .into_iter()
            .map(|x| SSeq::from_bytes(x)),
        );

        assert_equal(
            SSeq::from_bytes(b"GNT")
                .one_hamming_iter(HammingIterOpt::SkipNBase)
                .collect_vec(),
            vec![b"ANT", b"CNT", b"TNT", b"GNA", b"GNC", b"GNG"]
                .into_iter()
                .map(|x| SSeq::from_bytes(x)),
        );
    }

    #[test]
    fn test_from_iter() {
        let seq = SSeq::from_bytes(b"ACGT");
        let _ = SSeq::from_iter(seq.as_bytes());
        let seq_vec = seq.as_bytes().to_vec();
        let _ = SSeq::from_iter(seq_vec.into_iter());
    }
}
