use std::str;
use bio::stats::LogProb;
use rust_htslib::bam;
use rust_htslib::bam::record::CigarString;


/// An observation for or against a variant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    /// Posterior probability that the read/read-pair has been mapped correctly (1 - MAPQ).
    pub prob_mapping: LogProb,
    /// Probability that the read/read-pair comes from the alternative allele.
    pub prob_alt: LogProb,
    /// Probability that the read/read-pair comes from the reference allele.
    pub prob_ref: LogProb,
    /// Probability of the read/read-pair given that it has been mismapped.
    pub prob_mismapped: LogProb,
    /// Type of evidence.
    pub evidence: Evidence
}


impl Observation {
    pub fn is_alignment_evidence(&self) -> bool {
        if let Evidence::Alignment(_) = self.evidence {
            true
        } else {
            false
        }
    }
}


/// Types of evidence that lead to an observation.
/// The contained information is intended for debugging and will be printed together with
/// observations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Evidence {
    /// Insert size of fragment
    InsertSize(String),
    /// Alignment of a single read
    Alignment(String)
}


impl Evidence {
    /// Create a dummy alignment.
    pub fn dummy_alignment() -> Self {
        Evidence::Alignment("Dummy-Alignment".to_owned())
    }

    /// Create dummy insert size evidence.
    pub fn dummy_insert_size(insert_size: u32) -> Self {
        Evidence::InsertSize(format!("insert-size={}", insert_size))
    }

    /// Create insert size evidence.
    pub fn insert_size(
        insert_size: u32,
        left: &CigarString,
        right: &CigarString,
        left_record: &bam::Record,
        right_record: &bam::Record,
        p_left_ref: LogProb,
        p_left_alt: LogProb,
        p_right_ref: LogProb,
        p_right_alt: LogProb,
        p_isize_ref: LogProb,
        p_isize_alt: LogProb
    ) -> Self {
        Evidence::InsertSize(format!(
            "left: cigar={} ({:e} vs {:e}), right: cigar={} ({:e} vs {:e}), insert-size={} ({:e} vs {:e}), qname={}, left: AS={:?}, XS={:?}, right: AS={:?}, XS={:?}",
            left, p_left_ref.exp(), p_left_alt.exp(),
            right, p_right_ref.exp(), p_right_alt.exp(),
            insert_size, p_isize_ref.exp(), p_isize_alt.exp(),
            str::from_utf8(left_record.qname()).unwrap(),
            left_record.aux(b"AS").map(|a| a.integer()),
            left_record.aux(b"XS").map(|a| a.integer()),
            right_record.aux(b"AS").map(|a| a.integer()),
            right_record.aux(b"XS").map(|a| a.integer())
        ))
    }

    /// Create alignment evidence.
    pub fn alignment(cigar: &CigarString, record: &bam::Record) -> Self {
        Evidence::Alignment(format!(
            "cigar={}, qname={}, AS={:?}, XS={:?}",
            cigar, str::from_utf8(record.qname()).unwrap(),
            record.aux(b"AS").map(|a| a.integer()),
            record.aux(b"XS").map(|a| a.integer())
        ))
    }
}