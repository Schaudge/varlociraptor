
use std::error::Error;
use std::path::Path;
use rust_htslib::bcf;
use rust_htslib::bcf::Read;
use itertools::Itertools;
use bio::stats::bayesian::bayes_factors::{evidence::KassRaftery, BayesFactor};

use Event;
use utils;


/// Filter calls by posterior odds against the given events.
/// If odds against the events is at least the given `KassRaftery` score, remove allele.
pub fn filter_by_odds<E, R, W>(
    inbcf: Option<R>,
    outbcf: Option<W>,
    events: &[E],
    min_evidence: KassRaftery,
) -> Result<(), Box<Error>>
where
    E: Event,
    R: AsRef<Path>,
    W: AsRef<Path>,
{
    let mut inbcf_reader = match inbcf {
        Some(p) => bcf::Reader::from_path(p)?,
        None => bcf::Reader::from_stdin()?,
    };

    let other_event_tags = inbcf_reader.header().header_records().iter().filter_map(|rec| {
        if let bcf::header::HeaderRecord::Info { key: ref tag, .. } = rec {
            if tag.starts_with("PROB_") {
                Some(tag.to_owned())
            } else {
                None
            }
        } else {
            None
        }
    }).collect_vec();
    let event_tags = utils::events_to_tags(events);

    // setup output file
    let header = bcf::Header::from_template(inbcf_reader.header());
    let mut outbcf = match outbcf {
        Some(p) => bcf::Writer::from_path(p, &header, false, false)?,
        None => bcf::Writer::from_stdout(&header, false, false)?,
    };

    let filter = |record: &mut bcf::Record| {
        let target_probs = utils::tags_prob_sum(record, &event_tags, None)?;
        let other_probs = utils::tags_prob_sum(record, &other_event_tags, None)?;
        Ok(
            target_probs.into_iter().zip(other_probs.into_iter()).map(|probs| {
                match probs {
                    (Some(tp), Some(op)) => {
                        // If the odds for the other events are barely more likely or
                        // not at all more likely than the target event, keep the allele.
                        BayesFactor::new(op, tp).evidence_kass_raftery() < min_evidence
                    }
                    // Variant does not fit in given vartype.
                    (None, None) => false,
                    _ => panic!("bug: divergence in variant filtration")
                }
            })
        )
    };

    utils::filter_calls(&mut inbcf_reader, &mut outbcf, filter)
}
