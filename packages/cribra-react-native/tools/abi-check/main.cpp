// Host-side check for the pure ABI policy used by the native bridge: the
// major/minor gate, the JavaScript safe-integer boundary, and the strict
// enum/explanation projection. It includes only headers, so it needs no React
// Native toolchain.
//
// Build and run (also available as `npm run test:abi`):
//   clang++ -std=c++17 -I third_party/cribra/include \
//     tools/abi-check/main.cpp -o /tmp/cribra-abi-check && /tmp/cribra-abi-check

#include "../../cpp/CribraAbi.h"
#include "../../cpp/CribraEnums.h"

#include <cassert>
#include <cstdint>
#include <cstdio>
#include <string_view>

namespace {

using cribra::rn::FitsJsSafeInteger;
using cribra::rn::kMaxJsSafeInteger;
using cribra::rn::MapCandidateEvidence;
using cribra::rn::MapCandidateKind;
using cribra::rn::MapConfidence;
using cribra::rn::MapDetectionMode;
using cribra::rn::MapExplanation;
using cribra::rn::MapRemediation;
using cribra::rn::MapSeverity;
using cribra::rn::SeverityRank;

bool Has(const std::optional<cribra::rn::SeverityMapping>& mapping,
         std::string_view label,
         SeverityRank rank) {
  return mapping && mapping->label == label && mapping->rank == rank;
}

bool Has(const std::optional<std::string_view>& mapping,
         std::string_view label) {
  return mapping && *mapping == label;
}

bool Has(const std::optional<cribra::rn::ExplanationMapping>& mapping,
         std::string_view kind,
         std::string_view value) {
  return mapping && mapping->kind == kind && mapping->value == value;
}

void CheckAbiGate() {
  using cribra::rn::AbiCompatible;
  using cribra::rn::AbiVersionText;

  // The vendored header currently describes the 0.3 protocol.
  static_assert(cribra::rn::kExpectedAbiMajor == 0);
  static_assert(cribra::rn::kExpectedAbiMinor == 3);

  // Same protocol surface, including any patch difference.
  assert(AbiCompatible(0, 3, 0));
  assert(AbiCompatible(0, 3, 7));

  // Incompatible protocol surfaces fail closed.
  assert(!AbiCompatible(0, 2, 9));
  assert(!AbiCompatible(0, 4, 0));
  assert(!AbiCompatible(1, 3, 0));
  assert(!AbiCompatible(2, 0, 0));
  assert(!AbiCompatible(0, 0, 0));

  assert(AbiVersionText(0, 3, 0) == "0.3.0");
  assert(AbiVersionText(1, 0, 2) == "1.0.2");
}

void CheckJsSafeInteger() {
  static_assert(kMaxJsSafeInteger == 9007199254740991ull);
  assert(FitsJsSafeInteger(0));
  assert(FitsJsSafeInteger(1));
  assert(FitsJsSafeInteger(kMaxJsSafeInteger));

  // One past the largest exactly representable integer, and the uint64 limit,
  // must be rejected before any conversion to a JavaScript Number.
  assert(!FitsJsSafeInteger(kMaxJsSafeInteger + 1));
  assert(!FitsJsSafeInteger(UINT64_MAX));
}

void CheckFindingEnums() {
  assert(Has(MapSeverity(CRIBRA_SEVERITY_INFO), "info", SeverityRank::Info));
  assert(Has(MapSeverity(CRIBRA_SEVERITY_LOW), "low", SeverityRank::Low));
  assert(Has(
      MapSeverity(CRIBRA_SEVERITY_MEDIUM), "medium", SeverityRank::Medium));
  assert(Has(MapSeverity(CRIBRA_SEVERITY_HIGH), "high", SeverityRank::High));
  assert(Has(
      MapSeverity(CRIBRA_SEVERITY_CRITICAL),
      "critical",
      SeverityRank::Critical));

  // Severity has no sentinel: codes outside the documented range, including
  // 0xFFFFFFFF, fail closed.
  assert(!MapSeverity(5));
  assert(!MapSeverity(6));
  assert(!MapSeverity(0xFFFFFFFFu));

  assert(Has(MapConfidence(CRIBRA_CONFIDENCE_LOW), "low"));
  assert(Has(MapConfidence(CRIBRA_CONFIDENCE_MEDIUM), "medium"));
  assert(Has(MapConfidence(CRIBRA_CONFIDENCE_HIGH), "high"));
  // Confidence has no sentinel either.
  assert(!MapConfidence(3));
  assert(!MapConfidence(4));
  assert(!MapConfidence(0xFFFFFFFFu));

  assert(Has(MapRemediation(CRIBRA_REMEDIATION_NONE), ""));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL),
      "revoke_and_rotate_credential"));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_ROTATE_CREDENTIAL),
      "rotate_credential"));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_ROTATE_PASSWORD), "rotate_password"));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY),
      "replace_private_key"));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE),
      "remove_sensitive_value"));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH),
      "review_sensitive_hash"));
  assert(Has(
      MapRemediation(CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER),
      "review_password_verifier"));
  assert(Has(MapRemediation(CRIBRA_REMEDIATION_UNKNOWN), "unknown"));
  assert(!MapRemediation(8));
  assert(!MapRemediation(0xFFFFFFFEu));
}

void CheckCandidateEnums() {
  assert(Has(MapDetectionMode(CRIBRA_DETECTION_MODE_NONE), "none"));
  assert(Has(MapDetectionMode(CRIBRA_DETECTION_MODE_MATCHER_ONLY), "matcher_only"));
  assert(Has(
      MapDetectionMode(CRIBRA_DETECTION_MODE_DETERMINISTIC), "deterministic"));
  assert(Has(MapDetectionMode(CRIBRA_DETECTION_MODE_CONTEXTUAL), "contextual"));
  assert(Has(MapDetectionMode(CRIBRA_DETECTION_MODE_UNKNOWN), "unknown"));
  assert(!MapDetectionMode(4));
  assert(!MapDetectionMode(0xFFFFFFFEu));

  assert(Has(MapCandidateEvidence(CRIBRA_CANDIDATE_EVIDENCE_NONE), "none"));
  assert(Has(
      MapCandidateEvidence(CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL), "structural"));
  assert(Has(
      MapCandidateEvidence(CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN), "unknown"));
  assert(!MapCandidateEvidence(2));
  assert(!MapCandidateEvidence(0xFFFFFFFEu));

  assert(Has(MapCandidateKind(CRIBRA_CANDIDATE_KIND_NONE), "none"));
  assert(Has(
      MapCandidateKind(CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE),
      "recovery_like_code"));
  assert(Has(MapCandidateKind(CRIBRA_CANDIDATE_KIND_UNKNOWN), "unknown"));
  assert(!MapCandidateKind(2));
  assert(!MapCandidateKind(0xFFFFFFFEu));
}

void CheckExplanationMapping() {
  constexpr auto kNone = CRIBRA_DETECTION_MODE_NONE;
  constexpr auto kEvidenceNone = CRIBRA_CANDIDATE_EVIDENCE_NONE;
  constexpr auto kEvidenceStructural = CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL;
  constexpr auto kEvidenceUnknown = CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN;

  // Documented, consistent combinations.
  assert(Has(
      MapExplanation(CRIBRA_EXPLANATION_NONE, kNone, kEvidenceNone),
      "none",
      ""));
  assert(Has(
      MapExplanation(
          CRIBRA_EXPLANATION_CLASSIFIED,
          CRIBRA_DETECTION_MODE_MATCHER_ONLY,
          kEvidenceNone),
      "classified",
      "matcher_only"));
  assert(Has(
      MapExplanation(
          CRIBRA_EXPLANATION_CLASSIFIED,
          CRIBRA_DETECTION_MODE_DETERMINISTIC,
          kEvidenceNone),
      "classified",
      "deterministic"));
  assert(Has(
      MapExplanation(
          CRIBRA_EXPLANATION_CLASSIFIED,
          CRIBRA_DETECTION_MODE_CONTEXTUAL,
          kEvidenceNone),
      "classified",
      "contextual"));
  assert(Has(
      MapExplanation(
          CRIBRA_EXPLANATION_AMBIGUOUS, kNone, kEvidenceStructural),
      "ambiguous",
      "structural"));
  assert(Has(
      MapExplanation(CRIBRA_EXPLANATION_AMBIGUOUS, kNone, kEvidenceUnknown),
      "ambiguous",
      "unknown"));
  // The released C ABI emits UNKNOWN with both UNKNOWN sentinels; the header's
  // documented zero-value form (both NONE) is also accepted.
  assert(Has(
      MapExplanation(CRIBRA_EXPLANATION_UNKNOWN, kNone, kEvidenceNone),
      "unknown",
      ""));
  assert(Has(
      MapExplanation(
          CRIBRA_EXPLANATION_UNKNOWN,
          CRIBRA_DETECTION_MODE_UNKNOWN,
          kEvidenceUnknown),
      "unknown",
      ""));

  // Unrecognized explanation families fail closed.
  assert(!MapExplanation(3, kNone, kEvidenceNone));
  assert(!MapExplanation(0xFFFFFFFEu, kNone, kEvidenceNone));

  // Sentinels are accepted only where the header documents them. NONE/UNKNOWN
  // families must leave both payloads at NONE.
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_NONE, CRIBRA_DETECTION_MODE_DETERMINISTIC, kEvidenceNone));
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_NONE, kNone, kEvidenceStructural));
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_UNKNOWN, CRIBRA_DETECTION_MODE_DETERMINISTIC, kEvidenceNone));
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_UNKNOWN, kNone, kEvidenceStructural));
  // Mixed UNKNOWN/NONE payload pairs are impossible for the UNKNOWN family.
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_UNKNOWN,
      CRIBRA_DETECTION_MODE_UNKNOWN,
      kEvidenceNone));
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_UNKNOWN,
      kNone,
      kEvidenceUnknown));

  // CLASSIFIED needs a concrete mode and must leave evidence at NONE.
  assert(!MapExplanation(CRIBRA_EXPLANATION_CLASSIFIED, kNone, kEvidenceNone));
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_CLASSIFIED,
      CRIBRA_DETECTION_MODE_UNKNOWN,
      kEvidenceNone));
  assert(!MapExplanation(CRIBRA_EXPLANATION_CLASSIFIED, 4, kEvidenceNone));
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_CLASSIFIED,
      CRIBRA_DETECTION_MODE_DETERMINISTIC,
      kEvidenceStructural));

  // AMBIGUOUS needs concrete evidence and must leave the mode at NONE.
  assert(!MapExplanation(
      CRIBRA_EXPLANATION_AMBIGUOUS,
      CRIBRA_DETECTION_MODE_DETERMINISTIC,
      kEvidenceStructural));
  assert(!MapExplanation(CRIBRA_EXPLANATION_AMBIGUOUS, kNone, kEvidenceNone));
  assert(!MapExplanation(CRIBRA_EXPLANATION_AMBIGUOUS, kNone, 2));
}

}  // namespace

int main() {
  CheckAbiGate();
  CheckJsSafeInteger();
  CheckFindingEnums();
  CheckCandidateEnums();
  CheckExplanationMapping();

  std::puts("ABI compatibility gate: PASS");
  std::puts("JS safe-integer boundary: PASS");
  std::puts("Strict enum/explanation projection: PASS");
  return 0;
}
