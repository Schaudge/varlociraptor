use std::path::PathBuf;

use serde_json::json;
use yaml_rust::Yaml;

use crate::testcase::runner::common::Testcase;

#[derive(Debug)]
pub struct TestcaseVersion1 {
    pub inner: Vec<Yaml>,
    pub path: PathBuf,
}

impl Testcase for TestcaseVersion1 {
    fn inner(&self) -> &[Yaml] {
        &self.inner
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }

    fn sample_alignment_properties(&self, sample_name: &str) -> String {
        let mut props: serde_json::Value =
            serde_json::from_str(self.sample(sample_name)["properties"].as_str().unwrap()).unwrap();
        props.as_object_mut().unwrap().insert(
            "max_read_len".to_owned(),
            serde_json::Value::Number(serde_json::Number::from(100)),
        );

        props.to_string()
    }

    fn preprocess_options(&self, sample_name: &str) -> String {
        let mut options: serde_json::Value = serde_json::from_str(
            self.yaml()["samples"][sample_name]["options"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        let variants = options["Preprocess"]["kind"].get_mut("Variants").unwrap();
        variants["realignment_window"] = variants["indel_window"].clone();
        variants["candidates"] = json!("dummy.bcf");
        variants.as_object_mut().unwrap().remove("omit_snvs");
        variants.as_object_mut().unwrap().remove("omit_indels");
        variants["propagate_info_fields"] = json!([]);

        options.to_string()
    }
}
