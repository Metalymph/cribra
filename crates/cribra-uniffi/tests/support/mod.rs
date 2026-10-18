use cribra_uniffi::{ScanSource, SourceId};

pub fn sources() -> Vec<ScanSource> {
    let values: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/scanner.json")).unwrap();
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|value| ScanSource {
            id: SourceId {
                value: value["id"].as_str().unwrap().to_owned(),
            },
            text: value["text"].as_str().unwrap().to_owned(),
        })
        .collect()
}
