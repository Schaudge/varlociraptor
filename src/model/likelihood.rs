// Copyright 2016-2019 Johannes Köster, David Lähnemann.
// Licensed under the GNU GPLv3 license (https://opensource.org/licenses/GPL-3.0)
// This file may not be copied, modified, or distributed
// except according to those terms.

use bio::stats::{bayesian::model::Likelihood, LogProb};

use crate::model::evidence::Observation;
use crate::model::sample::Pileup;
use crate::model::AlleleFreq;

fn prob_sample_alt(observation: &Observation, allele_freq: LogProb) -> LogProb {
    if allele_freq != LogProb::ln_one() {
        // The effective sample probability for the alt allele is the allele frequency times
        // the probability to obtain a feasible fragment (prob_sample_alt).
        allele_freq + observation.prob_sample_alt
    } else {
        // If allele frequency is 1.0, sampling bias does have no effect because all reads
        // should come from the alt allele.
        allele_freq
    }
}

pub trait ContaminatedSamplePairView<T> {
    fn primary(&self) -> &T;
    fn secondary(&self) -> &T;
}

impl<T> ContaminatedSamplePairView<T> for Vec<T> {
    fn primary(&self) -> &T {
        &self[0]
    }

    fn secondary(&self) -> &T {
        &self[1]
    }
}

/// Variant calling model, taking purity and allele frequencies into account.
#[derive(Clone, Copy, Debug)]
pub struct ContaminatedSampleLikelihoodModel {
    /// Purity of the case sample.
    purity: LogProb,
    impurity: LogProb,
}

impl Default for ContaminatedSampleLikelihoodModel {
    fn default() -> Self {
        ContaminatedSampleLikelihoodModel::new(1.0)
    }
}

impl ContaminatedSampleLikelihoodModel {
    /// Create new model.
    pub fn new(purity: f64) -> Self {
        assert!(purity > 0.0 && purity <= 1.0);
        let purity = LogProb(purity.ln());
        ContaminatedSampleLikelihoodModel {
            purity: purity,
            impurity: purity.ln_one_minus_exp(),
        }
    }

    fn likelihood_observation(
        &self,
        allele_freq_primary: LogProb,
        allele_freq_secondary: LogProb,
        observation: &Observation,
    ) -> LogProb {
        // Step 1: likelihoods for the mapping case.
        // Case 1: read comes from primary sample and is correctly mapped
        let prob_primary = self.purity + likelihood_mapping(allele_freq_primary, observation);
        // Case 2: read comes from secondary sample and is correctly mapped
        let prob_secondary = self.impurity + likelihood_mapping(allele_freq_secondary, observation);

        // Step 4: total probability
        // Important note: we need to multiply a probability for a hypothetical missed allele
        // in the mismapping case. Otherwise, it can happen that mismapping dominates subtle
        // differences in the likelihood for alt and ref allele with low probabilities and very
        // low allele frequencies, such that we loose sensitivity for those.
        let total = (observation.prob_mapping + prob_secondary.ln_add_exp(prob_primary))
            .ln_add_exp(observation.prob_mismapping + observation.prob_missed_allele);
        assert!(!total.is_nan());
        total
    }
}

impl Likelihood for ContaminatedSampleLikelihoodModel {
    type Event = Vec<AlleleFreq>;
    type Data = Pileup;

    fn compute(&self, allelefreqs: &Self::Event, pileup: &Self::Data) -> LogProb {
        let ln_af_primary = LogProb(allelefreqs.primary().ln());
        let ln_af_secondary = LogProb(allelefreqs.secondary().ln());
        // calculate product of per-oservation likelihoods in log space
        let likelihood = pileup.iter().fold(LogProb::ln_one(), |prob, obs| {
            let lh = self.likelihood_observation(ln_af_primary, ln_af_secondary, obs);
            prob + lh
        });

        assert!(!likelihood.is_nan());
        likelihood
    }
}

lazy_static! {
    static ref PROB_STRAND: LogProb = LogProb(0.5_f64.ln());
}

/// Likelihood model for single sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct SampleLikelihoodModel {}

impl SampleLikelihoodModel {
    /// Create new model.
    pub fn new() -> Self {
        SampleLikelihoodModel {}
    }

    /// Likelihood to observe a read given allele frequency for a single sample.
    fn likelihood_observation(&self, allele_freq: LogProb, observation: &Observation) -> LogProb {
        // Step 1: likelihood for the mapping case.
        let prob = likelihood_mapping(allele_freq, observation);

        // Step 2: total probability
        // Important note: we need to multiply a probability for a hypothetical missed allele
        // in the mismapping case. Otherwise, it can happen that mismapping dominates subtle
        // differences in the likelihood for alt and ref allele with low probabilities and very
        // low allele frequencies, such that we loose sensitivity for those.
        let total = (observation.prob_mapping + prob)
            .ln_add_exp(observation.prob_mismapping + observation.prob_missed_allele);
        assert!(!total.is_nan());
        total
    }
}

/// Calculate likelihood of allele freq given observation in a single sample assuming that the
/// underlying fragment/read is mapped correctly.
fn likelihood_mapping(allele_freq: LogProb, observation: &Observation) -> LogProb {
    // Step 1: calculate probability to sample from alt allele
    let prob_sample_alt = prob_sample_alt(observation, allele_freq);
    let prob_sample_ref = prob_sample_alt.ln_one_minus_exp();

    // Step 2: read comes from case sample and is correctly mapped
    let prob = LogProb::ln_sum_exp(&[
        // alt allele forward strand
        *PROB_STRAND + prob_sample_alt + observation.prob_alt_forward(),
        // alt allele reverse strand
        *PROB_STRAND + prob_sample_alt + observation.prob_alt_reverse(),
        // ref allele forward strand
        *PROB_STRAND + prob_sample_ref + observation.prob_ref_forward(),
        // ref allele reverse strand
        *PROB_STRAND + prob_sample_ref + observation.prob_ref_reverse(),
    ]);
    assert!(!prob.is_nan());

    prob
}

impl Likelihood for SampleLikelihoodModel {
    type Event = AlleleFreq;
    type Data = Pileup;

    /// Likelihood to observe a pileup given allele frequencies for case and control.
    fn compute(&self, allele_freq: &AlleleFreq, pileup: &Pileup) -> LogProb {
        let ln_af = LogProb(allele_freq.ln());
        // calculate product of per-read likelihoods in log space
        let likelihood = pileup.iter().fold(LogProb::ln_one(), |prob, obs| {
            let lh = self.likelihood_observation(ln_af, obs);
            prob + lh
        });

        assert!(!likelihood.is_nan());
        likelihood
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::observation;
    use bio::stats::LogProb;
    use itertools_num::linspace;

    #[test]
    fn test_likelihood_observation_absent_single() {
        let observation = observation(LogProb::ln_one(), LogProb::ln_zero(), LogProb::ln_one());

        let model = SampleLikelihoodModel::new();

        let lh = model.likelihood_observation(LogProb(AlleleFreq(0.0).ln()), &observation);
        assert_relative_eq!(*lh, *LogProb::ln_one());
    }

    #[test]
    fn test_likelihood_observation_absent() {
        let model = ContaminatedSampleLikelihoodModel::new(1.0);
        let observation = observation(LogProb::ln_one(), LogProb::ln_zero(), LogProb::ln_one());

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(0.0).ln()),
            LogProb(AlleleFreq(0.0).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, *LogProb::ln_one());
    }

    #[test]
    fn test_likelihood_pileup_absent() {
        let model = ContaminatedSampleLikelihoodModel::new(1.0);
        let mut observations = Vec::new();
        for _ in 0..10 {
            observations.push(observation(
                LogProb::ln_one(),
                LogProb::ln_zero(),
                LogProb::ln_one(),
            ));
        }

        let lh = model.compute(&vec![AlleleFreq(0.0), AlleleFreq(0.0)], &observations);
        assert_relative_eq!(*lh, *LogProb::ln_one());
    }

    #[test]
    fn test_likelihood_pileup_absent_single() {
        let model = SampleLikelihoodModel::new();
        let mut observations = Vec::new();
        for _ in 0..10 {
            observations.push(observation(
                LogProb::ln_one(),
                LogProb::ln_zero(),
                LogProb::ln_one(),
            ));
        }

        let lh = model.compute(&AlleleFreq(0.0), &observations);
        assert_relative_eq!(*lh, *LogProb::ln_one());
    }

    #[test]
    fn test_likelihood_observation_case_control() {
        let model = ContaminatedSampleLikelihoodModel::new(1.0);
        let observation = observation(LogProb::ln_one(), LogProb::ln_one(), LogProb::ln_zero());

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(1.0).ln()),
            LogProb(AlleleFreq(0.0).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, *LogProb::ln_one());

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(0.0).ln()),
            LogProb(AlleleFreq(0.0).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, *LogProb::ln_zero());

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(0.5).ln()),
            LogProb(AlleleFreq(0.0).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, 0.5f64.ln());

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(0.5).ln()),
            LogProb(AlleleFreq(0.5).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, 0.5f64.ln());

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(0.1).ln()),
            LogProb(AlleleFreq(0.0).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, 0.1f64.ln());

        // test with 50% purity
        let model = ContaminatedSampleLikelihoodModel::new(0.5);

        let lh = model.likelihood_observation(
            LogProb(AlleleFreq(0.0).ln()),
            LogProb(AlleleFreq(1.0).ln()),
            &observation,
        );
        assert_relative_eq!(*lh, 0.5f64.ln(), epsilon = 0.0000000001);
    }

    #[test]
    fn test_likelihood_observation_single_sample() {
        let model = SampleLikelihoodModel::new();

        let observation = observation(
            // prob_mapping
            LogProb::ln_one(),
            // prob_alt
            LogProb::ln_one(),
            // prob_ref
            LogProb::ln_zero(),
        );

        let lh = model.likelihood_observation(LogProb(AlleleFreq(1.0).ln()), &observation);
        assert_relative_eq!(*lh, *LogProb::ln_one());

        let lh = model.likelihood_observation(LogProb(AlleleFreq(0.0).ln()), &observation);
        assert_relative_eq!(*lh, *LogProb::ln_zero());

        let lh = model.likelihood_observation(LogProb(AlleleFreq(0.5).ln()), &observation);
        assert_relative_eq!(*lh, 0.5f64.ln());

        let lh = model.likelihood_observation(LogProb(AlleleFreq(0.1).ln()), &observation);
        assert_relative_eq!(*lh, 0.1f64.ln());
    }

    #[test]
    fn test_likelihood_pileup() {
        let model = ContaminatedSampleLikelihoodModel::new(1.0);
        let mut observations = Vec::new();
        for _ in 0..5 {
            observations.push(observation(
                LogProb::ln_one(),
                LogProb::ln_one(),
                LogProb::ln_zero(),
            ));
        }
        for _ in 0..5 {
            observations.push(observation(
                LogProb::ln_one(),
                LogProb::ln_zero(),
                LogProb::ln_one(),
            ));
        }
        let lh = model.compute(&vec![AlleleFreq(0.5), AlleleFreq(0.0)], &observations);
        for af in linspace(0.0, 1.0, 10) {
            if af != 0.5 {
                let l = model.compute(&vec![AlleleFreq(af), AlleleFreq(0.0)], &observations);
                assert!(lh > l);
            }
        }
    }
}
