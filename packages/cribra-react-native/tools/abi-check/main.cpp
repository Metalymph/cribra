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
#include <type_traits>

namespace {

using cribra::rn::FitsJsSafeInteger;
using cribra::rn::IsCanonicalDefaultConfig;
using cribra::rn::kMaxJsSafeInteger;
using cribra::rn::MapBuildErrorCode;
using cribra::rn::MapCandidateEvidence;
using cribra::rn::MapCandidateKind;
using cribra::rn::MapConfidence;
using cribra::rn::MapDetectionMode;
using cribra::rn::MapExplanation;
using cribra::rn::MapRemediation;
using cribra::rn::MapRemediationCode;
using cribra::rn::MapRuleDetectionMode;
using cribra::rn::MapRuleKind;
using cribra::rn::MapRuleKindCode;
using cribra::rn::MapSeverity;
using cribra::rn::MapSeverityCode;
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

void CheckTransformProtocol() {
  // The transformation surface is part of the same 0.3 ABI. Pin the exported
  // signatures so a header or linkage drift fails this host check.
  using RedactFn = CribraStatus (*)(
      const uint8_t*,
      size_t,
      const CribraReport*,
      CribraOutput**,
      CribraError**);
  using RedactWithFn = CribraStatus (*)(
      const uint8_t*,
      size_t,
      const CribraReport*,
      const uint8_t*,
      size_t,
      CribraOutput**,
      CribraError**);
  using TemplateFn = CribraStatus (*)(
      const uint8_t*,
      size_t,
      const CribraReport*,
      CribraOutput**,
      CribraError**);
  using TemplateWithFn = CribraStatus (*)(
      const uint8_t*,
      size_t,
      const CribraReport*,
      const CribraTemplateConfig*,
      CribraOutput**,
      CribraError**);
  using PseudonymizeFn = CribraStatus (*)(
      const uint8_t*,
      size_t,
      const CribraReport*,
      const CribraPseudonymizeConfig*,
      CribraOutput**,
      CribraError**);
  using SynthesizeFn = CribraStatus (*)(
      const uint8_t*,
      size_t,
      const CribraReport*,
      const CribraSynthesizeConfig*,
      CribraOutput**,
      CribraError**);

  static_assert(std::is_same_v<decltype(&cribra_transform_redact), RedactFn>);
  static_assert(
      std::is_same_v<decltype(&cribra_transform_redact_with), RedactWithFn>);
  static_assert(
      std::is_same_v<decltype(&cribra_transform_template), TemplateFn>);
  static_assert(
      std::is_same_v<decltype(&cribra_transform_template_with), TemplateWithFn>);
  static_assert(std::is_same_v<
                decltype(&cribra_transform_pseudonymize),
                PseudonymizeFn>);
  static_assert(
      std::is_same_v<decltype(&cribra_transform_synthesize), SynthesizeFn>);

  // Transform failures use their own coarse status; classification in the
  // facade is a compatibility mapping over fixed diagnostics, not this code.
  static_assert(CRIBRA_TRANSFORM_ERROR == 5);

  // The three configuration structs used by the bridge carry exactly the
  // documented fields and borrow caller memory only for one call.
  CribraTemplateConfig templateConfig{};
  templateConfig.namespace_ = CribraStringView{nullptr, 0};
  templateConfig.numbered = 0;
  assert(templateConfig.numbered == 0);

  CribraPseudonymizeConfig pseudonymizeConfig{};
  pseudonymizeConfig.key = nullptr;
  pseudonymizeConfig.key_len = 0;
  pseudonymizeConfig.prefix = CribraStringView{nullptr, 0};
  pseudonymizeConfig.digest_bytes = 16;
  assert(pseudonymizeConfig.key_len == 0);
  assert(pseudonymizeConfig.digest_bytes == 16);

  CribraSynthesizeConfig synthesizeConfig{};
  synthesizeConfig.key = nullptr;
  synthesizeConfig.key_len = 0;
  synthesizeConfig.marker = CribraStringView{nullptr, 0};
  assert(synthesizeConfig.key_len == 0);

  // The output handle is Rust-owned and released exactly once.
  using OutputFreeFn = void (*)(CribraOutput*);
  using OutputViewFn =
      CribraStatus (*)(const CribraOutput*, CribraStringView*);
  static_assert(std::is_same_v<decltype(&cribra_output_free), OutputFreeFn>);
  static_assert(std::is_same_v<decltype(&cribra_output_view), OutputViewFn>);
}

void CheckConfigurationProtocol() {
  // Rule metadata has no sentinel: all four documented families map, and any
  // other value fails closed.
  assert(Has(MapRuleKind(CRIBRA_RULE_KIND_LITERAL), "literal"));
  assert(Has(MapRuleKind(CRIBRA_RULE_KIND_PREFIX), "prefix"));
  assert(Has(MapRuleKind(CRIBRA_RULE_KIND_SUFFIX), "suffix"));
  assert(Has(MapRuleKind(CRIBRA_RULE_KIND_PATTERN), "pattern"));
  assert(!MapRuleKind(4));
  assert(!MapRuleKind(0xFFFFFFFFu));

  // Active rules carry a concrete authority. Explanation sentinels are not
  // valid metadata.
  assert(Has(
      MapRuleDetectionMode(CRIBRA_DETECTION_MODE_MATCHER_ONLY),
      "matcher_only"));
  assert(Has(
      MapRuleDetectionMode(CRIBRA_DETECTION_MODE_DETERMINISTIC),
      "deterministic"));
  assert(Has(
      MapRuleDetectionMode(CRIBRA_DETECTION_MODE_CONTEXTUAL), "contextual"));
  assert(!MapRuleDetectionMode(CRIBRA_DETECTION_MODE_NONE));
  assert(!MapRuleDetectionMode(CRIBRA_DETECTION_MODE_UNKNOWN));
  assert(!MapRuleDetectionMode(4));

  // Input vocabulary.
  assert(MapRuleKindCode("literal") == CRIBRA_RULE_KIND_LITERAL);
  assert(MapRuleKindCode("prefix") == CRIBRA_RULE_KIND_PREFIX);
  assert(MapRuleKindCode("suffix") == CRIBRA_RULE_KIND_SUFFIX);
  assert(MapRuleKindCode("pattern") == CRIBRA_RULE_KIND_PATTERN);
  assert(!MapRuleKindCode("regex"));
  assert(!MapRuleKindCode(""));

  assert(MapSeverityCode("info") == CRIBRA_SEVERITY_INFO);
  assert(MapSeverityCode("low") == CRIBRA_SEVERITY_LOW);
  assert(MapSeverityCode("medium") == CRIBRA_SEVERITY_MEDIUM);
  assert(MapSeverityCode("high") == CRIBRA_SEVERITY_HIGH);
  assert(MapSeverityCode("critical") == CRIBRA_SEVERITY_CRITICAL);
  assert(!MapSeverityCode("severe"));
  assert(!MapSeverityCode(""));

  assert(MapRemediationCode("") == CRIBRA_REMEDIATION_NONE);
  assert(
      MapRemediationCode("revoke_and_rotate_credential") ==
      CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL);
  assert(
      MapRemediationCode("rotate_credential") ==
      CRIBRA_REMEDIATION_ROTATE_CREDENTIAL);
  assert(
      MapRemediationCode("rotate_password") ==
      CRIBRA_REMEDIATION_ROTATE_PASSWORD);
  assert(
      MapRemediationCode("replace_private_key") ==
      CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY);
  assert(
      MapRemediationCode("remove_sensitive_value") ==
      CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE);
  assert(
      MapRemediationCode("review_sensitive_hash") ==
      CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH);
  assert(
      MapRemediationCode("review_password_verifier") ==
      CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER);
  // UNKNOWN is an output sentinel; Core rejects it as an input remediation.
  assert(!MapRemediationCode("unknown"));
  assert(!MapRemediationCode("none"));
  assert(!MapRemediationCode("delete"));

  // Numeric build codes are a stable ABI contract.
  static_assert(CRIBRA_BUILD_CODE_NONE == 0);
  static_assert(CRIBRA_BUILD_CODE_INVALID_PATTERN == 1);
  static_assert(CRIBRA_BUILD_CODE_PATTERN_MATCHES_EMPTY == 2);
  static_assert(CRIBRA_BUILD_CODE_MISSING_CAPTURE_GROUP == 3);
  static_assert(CRIBRA_BUILD_CODE_EMPTY_RULE_ID == 4);
  static_assert(CRIBRA_BUILD_CODE_DUPLICATE_RULE_ID == 5);
  static_assert(CRIBRA_BUILD_CODE_EMPTY_MATCHER == 6);
  static_assert(CRIBRA_BUILD_CODE_TOO_MANY_RULES == 7);
  static_assert(CRIBRA_BUILD_CODE_AUTOMATON_BUILD == 8);
  assert(Has(MapBuildErrorCode(0), "none"));
  assert(Has(MapBuildErrorCode(1), "invalid_pattern"));
  assert(Has(MapBuildErrorCode(2), "pattern_matches_empty"));
  assert(Has(MapBuildErrorCode(3), "missing_capture_group"));
  assert(Has(MapBuildErrorCode(4), "empty_rule_id"));
  assert(Has(MapBuildErrorCode(5), "duplicate_rule_id"));
  assert(Has(MapBuildErrorCode(6), "empty_matcher"));
  assert(Has(MapBuildErrorCode(7), "too_many_rules"));
  assert(Has(MapBuildErrorCode(8), "automaton_build"));
  assert(!MapBuildErrorCode(9));
  assert(!MapBuildErrorCode(0xFFFFFFFFu));

  // Only the canonical catalog with no other source takes the default fast
  // path and therefore matches `Scanner::default()`.
  assert(IsCanonicalDefaultConfig(true, false, false, false));
  assert(!IsCanonicalDefaultConfig(false, false, false, false));
  assert(!IsCanonicalDefaultConfig(true, true, false, false));
  assert(!IsCanonicalDefaultConfig(true, false, true, false));
  assert(!IsCanonicalDefaultConfig(true, false, false, true));
  assert(!IsCanonicalDefaultConfig(false, true, true, true));
}

}  // namespace

int main() {
  CheckAbiGate();
  CheckJsSafeInteger();
  CheckFindingEnums();
  CheckCandidateEnums();
  CheckExplanationMapping();
  CheckTransformProtocol();
  CheckConfigurationProtocol();

  std::puts("ABI compatibility gate: PASS");
  std::puts("JS safe-integer boundary: PASS");
  std::puts("Strict enum/explanation projection: PASS");
  std::puts("Transformation ABI signatures: PASS");
  std::puts("Scanner configuration protocol: PASS");
  return 0;
}
