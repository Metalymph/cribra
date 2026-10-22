//! Sequential multi-source scanning delegated to one Core batch invocation.
//!
//! The adapter copies the two caller-supplied input fields exactly once at the
//! boundary, hands the complete batch to [`cribra::Scanner::scan`] and then
//! drops the input text. Returned batches, entries, reports and summaries own
//! metadata only; no result retains source text or a borrow of this engine.

use wasm_bindgen::prelude::*;

use crate::{FindingSeverity, ScanEngine, ScanResult, Severity};

#[wasm_bindgen(typescript_custom_section)]
const BATCH_TYPES: &str = r#"
/** One explicitly identified UTF-8 source. IDs are preserved exactly and never interpreted. */
export interface ScanBatchSource {
    readonly id: string;
    readonly source: string;
}
"#;

#[wasm_bindgen]
extern "C" {
    /// Input-only view of one batch element. Raw properties are validated before
    /// Core receives a copied source.
    #[wasm_bindgen(typescript_type = "ScanBatchSource")]
    type ScanBatchSource;
    #[wasm_bindgen(method, getter)]
    fn id(this: &ScanBatchSource) -> JsValue;
    #[wasm_bindgen(method, getter)]
    fn source(this: &ScanBatchSource) -> JsValue;
}

#[wasm_bindgen]
impl ScanEngine {
    /// Scans several identified UTF-8 sources in input order with exactly one
    /// Core `Scanner::scan` call.
    ///
    /// Empty IDs, Unicode IDs and duplicate IDs are preserved verbatim. The
    /// caller owns the input array, ID strings and source strings; this method
    /// does not retain them. It returns an independently owned
    /// [`ScanBatchResult`] with one entry per input and Core's aggregate summary.
    #[wasm_bindgen(js_name = scanBatch)]
    pub fn scan_batch(
        &self,
        #[wasm_bindgen(unchecked_param_type = "ScanBatchSource[]")] sources: Vec<JsValue>,
    ) -> Result<ScanBatchResult, JsError> {
        let inputs = decode_sources(&sources, "batch source")?;

        Ok(self.scan_batch_core(inputs))
    }
}

impl ScanEngine {
    /// Native-testable batch path. The complete ordered batch reaches Core once;
    /// this function performs projections only and never scans per entry.
    fn scan_batch_core(&self, inputs: Vec<(String, String)>) -> ScanBatchResult {
        let (ids, sources): (Vec<String>, Vec<String>) = inputs.into_iter().unzip();
        let results = self
            .scanner
            .scan(ids.into_iter().zip(sources.iter().map(String::as_str)));
        let summary = results.summary();
        let entries = results
            .into_iter()
            .map(|entry| {
                let (id, source_bytes, report) = entry.into_parts();

                ScanBatchEntry {
                    id,
                    source_bytes,
                    report,
                }
            })
            .collect();

        ScanBatchResult { entries, summary }
    }
}

pub(crate) fn decode_sources(
    values: &[JsValue],
    context: &str,
) -> Result<Vec<(String, String)>, JsError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| decode_source(value, index, context))
        .collect()
}

fn decode_source(
    value: &JsValue,
    index: usize,
    context: &str,
) -> Result<(String, String), JsError> {
    if !value.is_object() {
        return Err(JsError::new(&format!(
            "{context} at index {index} must be an object"
        )));
    }

    let input: &ScanBatchSource = value.unchecked_ref();
    let id = input.id().as_string().ok_or_else(|| {
        JsError::new(&format!("{context} at index {index} must have a string id"))
    })?;
    let source = input.source().as_string().ok_or_else(|| {
        JsError::new(&format!(
            "{context} at index {index} must have a string source"
        ))
    })?;

    Ok((id, source))
}

/// Owned result of one sequential batch scan.
///
/// The batch owns its entries and Core-derived summary. `entryAt()` returns a
/// fresh independently owned entry, and `summary()` returns a fresh
/// independently owned summary view. Free this batch once; its entries and
/// summaries remain valid when this batch or the creating engine is freed.
#[wasm_bindgen]
pub struct ScanBatchResult {
    entries: Vec<ScanBatchEntry>,
    summary: cribra::ScanSummary,
}

#[wasm_bindgen]
impl ScanBatchResult {
    /// Number of entries, always equal to the number of input sources.
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.entries.len()
    }

    /// Whether the input batch contained no sources.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a fresh independently owned entry at the preserved input index.
    #[wasm_bindgen(js_name = entryAt)]
    pub fn entry_at(&self, index: usize) -> Result<ScanBatchEntry, JsError> {
        self.entries
            .get(index)
            .cloned()
            .ok_or_else(|| JsError::new("batch entry index out of range"))
    }

    /// Returns a fresh independently owned projection of Core's aggregate
    /// summary. It contains counters only and never retains source text.
    pub fn summary(&self) -> ScanSummaryView {
        ScanSummaryView::from(self.summary)
    }
}

/// Independently owned metadata for one source in a batch.
///
/// The entry owns its caller-supplied ID and report metadata, never source text.
/// Every `result` access returns a new [`ScanResult`] projection so no entry
/// hands out a borrowed or dangling reference. Free this entry and each returned
/// result separately.
#[wasm_bindgen]
#[derive(Clone)]
pub struct ScanBatchEntry {
    id: String,
    source_bytes: usize,
    report: cribra::ScanReport,
}

#[wasm_bindgen]
impl ScanBatchEntry {
    /// Returns the caller-supplied ID exactly as received.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Returns the original source's UTF-8 byte length.
    #[wasm_bindgen(getter, js_name = sourceBytes)]
    pub fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    /// Returns a fresh, complete, independently owned existing [`ScanResult`]
    /// projection. Each access clones report metadata; save and free every
    /// returned result independently. No source text is retained.
    #[wasm_bindgen(getter)]
    pub fn result(&self) -> ScanResult {
        ScanResult {
            source_bytes: self.source_bytes,
            report: self.report.clone(),
        }
    }
}

/// Owned projection of Core's aggregate `ScanSummary`.
///
/// Every value comes directly from Core's summary for the complete batch. The
/// view owns counters only and never stores source text, findings, source IDs or
/// matched values. Free every view separately.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ScanSummaryView {
    summary: cribra::ScanSummary,
}

impl From<cribra::ScanSummary> for ScanSummaryView {
    fn from(summary: cribra::ScanSummary) -> Self {
        Self { summary }
    }
}

#[wasm_bindgen]
impl ScanSummaryView {
    /// Number of scanned sources.
    #[wasm_bindgen(getter, js_name = scannedSources)]
    pub fn scanned_sources(&self) -> usize {
        self.summary.scanned_sources()
    }

    /// Total number of UTF-8 source bytes scanned.
    #[wasm_bindgen(getter, js_name = scannedBytes)]
    pub fn scanned_bytes(&self) -> usize {
        self.summary.scanned_bytes()
    }

    /// Source reports containing at least one finding.
    #[wasm_bindgen(getter, js_name = reportsWithFindings)]
    pub fn reports_with_findings(&self) -> usize {
        self.summary.reports_with_findings()
    }

    /// Source reports containing no findings.
    #[wasm_bindgen(getter, js_name = reportsWithoutFindings)]
    pub fn reports_without_findings(&self) -> usize {
        self.summary.reports_without_findings()
    }

    /// Total findings across all sources.
    #[wasm_bindgen(getter, js_name = totalFindings)]
    pub fn total_findings(&self) -> usize {
        self.summary.total_findings()
    }

    /// Total ambiguous sensitive candidates across all sources.
    #[wasm_bindgen(getter, js_name = totalCandidates)]
    pub fn total_candidates(&self) -> usize {
        self.summary.total_candidates()
    }

    /// Reports containing at least one ambiguous sensitive candidate.
    #[wasm_bindgen(getter, js_name = reportsWithCandidates)]
    pub fn reports_with_candidates(&self) -> usize {
        self.summary.reports_with_candidates()
    }

    /// Whether the batch contains at least one ambiguous sensitive candidate.
    #[wasm_bindgen(getter, js_name = hasCandidates)]
    pub fn has_candidates(&self) -> bool {
        self.summary.has_candidates()
    }

    /// Number of critical findings.
    #[wasm_bindgen(getter)]
    pub fn critical(&self) -> usize {
        self.summary.critical()
    }

    /// Number of high-severity findings.
    #[wasm_bindgen(getter)]
    pub fn high(&self) -> usize {
        self.summary.high()
    }

    /// Number of medium-severity findings.
    #[wasm_bindgen(getter)]
    pub fn medium(&self) -> usize {
        self.summary.medium()
    }

    /// Number of low-severity findings.
    #[wasm_bindgen(getter)]
    pub fn low(&self) -> usize {
        self.summary.low()
    }

    /// Number of informational findings.
    #[wasm_bindgen(getter)]
    pub fn info(&self) -> usize {
        self.summary.info()
    }

    /// Whether the batch contains neither findings nor ambiguous candidates.
    #[wasm_bindgen(getter, js_name = isClean)]
    pub fn is_clean(&self) -> bool {
        self.summary.is_clean()
    }

    /// Whether the batch contains at least one critical finding.
    #[wasm_bindgen(getter, js_name = hasCritical)]
    pub fn has_critical(&self) -> bool {
        self.summary.has_critical()
    }

    /// Number of findings with exactly the requested severity.
    #[wasm_bindgen(js_name = bySeverity)]
    pub fn by_severity(&self, severity: FindingSeverity) -> usize {
        self.summary.by_severity(Severity::from(severity))
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
