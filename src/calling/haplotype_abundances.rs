use anyhow::Result;
use derive_builder::Builder;
use hdf5;
use kernel_density;
use rust_htslib::bcf;
use serde_json::json;

#[derive(Builder)]
#[builder(pattern = "owned")]
pub(crate) struct Caller {
    hdf5_reader: hdf5::File,
    vcf_reader: bcf::Reader,
    min_tpm: u64,
    qc_plot: Option<PathBuf>
}

pub(crate) struct ECDF {
    seqname: String,
    bootstraps: Vec<u64>,
    ecdf: Vec<f64>,
}

impl Caller {  
    //Below function outputs a vector of filtered seqnames according to min_tpm.
    pub(crate) fn filter_seqnames(&self) -> Result<Vec<String>> {
        let hdf5 = &self.hdf5_reader;
        let tpm = hdf5.dataset("bootstrap/bs0")?.read_1d::<u64>()?;
        let mut indices = Vec::new();
        for (i,num) in tpm.iter().enumerate() {
            if num > &self.min_tpm {
                indices.push(i);
            }
        }
        let ids = hdf5
            .dataset("aux/ids")?
            .read_1d::<hdf5::types::VarLenAscii>()?;

        let mut filtered: Vec<String> = Vec::new();
        for i in indices {
            filtered.push(ids[i].to_string());
        }
        Ok(filtered)
    }

    //Below function outputs an ECDF struct for one haplotype.
    pub(crate) fn cdf(&self, seqname:String) -> Result<ECDF> {
        let hdf5 = &self.hdf5_reader;
        let ids = hdf5
            .dataset("aux/ids")?
            .read_1d::<hdf5::types::VarLenAscii>()?;
        let index = ids.iter().position(|x| x.as_str() == seqname).unwrap();
        let num_bootstraps = hdf5.dataset("/aux/num_bootstrap")?.read_scalar()?;
        let mut bootstraps = Vec::new();
        for i in 0..num_bootstraps {
            let dataset = hdf5.dataset(&format!("bootstrap/bs{i}", i = i))?;
            let tpm = dataset.read_1d::<u64>()?;
            let tst = tpm[index]; //a better variable name
            bootstraps.push(tst);
        }
        let mut ecdf = kernel_density::ecdf::Ecdf::new(&bootstraps);
        let ecdf: Vec<f64> = bootstraps.iter().map(|&x| ecdf.value(x)).collect();
        Ok(ECDF {
            seqname,
            bootstraps,
            ecdf,
        })
    
    }

    pub(crate) fn call(&self) {
        todo!()
    }
}

impl ECDF {
    pub(crate) fn qc(&self) -> Result<()> {
        let blueprint = "../../templates/plots/qc_cdf.json";
        let mut blueprint: serde_json::Value = serde_json::from_str(blueprint)?;

        let bootstraps = &self.bootstraps;
        let ecdf = &self.ecdf;

        let data = json!([bootstraps, ecdf]);
        blueprint["data"]["values"] = data;

        println!("{}", serde_json::to_string_pretty(&blueprint)?);

        Ok(())
    } 
}
