#pragma once

#include <cribra.h>

#include <cstdint>
#include <optional>
#include <string_view>
#include <utility>

namespace cribra::rn {

// Pure ABI enum projection, shared by the bridge and the host protocol check.
//
// Each mapper distinguishes two cases:
//   - a documented value (including ABI sentinels such as NONE/UNKNOWN) maps to
//     a label;
//   - a genuinely unrecognized numeric code returns std::nullopt so the bridge
//     can fail closed with a source-free protocol error.
//
// Labels are the wire vocabulary already used by the React Native DTOs.

enum class SeverityRank : uint32_t { Info, Low, Medium, High, Critical };

struct SeverityMapping {
  std::string_view label;
  SeverityRank rank;
};

inline std::optional<SeverityMapping> MapSeverity(uint32_t code) {
  switch (code) {
    case CRIBRA_SEVERITY_INFO:
      return SeverityMapping{"info", SeverityRank::Info};
    case CRIBRA_SEVERITY_LOW:
      return SeverityMapping{"low", SeverityRank::Low};
    case CRIBRA_SEVERITY_MEDIUM:
      return SeverityMapping{"medium", SeverityRank::Medium};
    case CRIBRA_SEVERITY_HIGH:
      return SeverityMapping{"high", SeverityRank::High};
    case CRIBRA_SEVERITY_CRITICAL:
      return SeverityMapping{"critical", SeverityRank::Critical};
    default:
      return std::nullopt;
  }
}

inline std::optional<std::string_view> MapConfidence(uint32_t code) {
  switch (code) {
    case CRIBRA_CONFIDENCE_LOW:
      return "low";
    case CRIBRA_CONFIDENCE_MEDIUM:
      return "medium";
    case CRIBRA_CONFIDENCE_HIGH:
      return "high";
    default:
      return std::nullopt;
  }
}

// An empty label is the wire representation of CRIBRA_REMEDIATION_NONE.
inline std::optional<std::string_view> MapRemediation(uint32_t code) {
  switch (code) {
    case CRIBRA_REMEDIATION_NONE:
      return "";
    case CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL:
      return "revoke_and_rotate_credential";
    case CRIBRA_REMEDIATION_ROTATE_CREDENTIAL:
      return "rotate_credential";
    case CRIBRA_REMEDIATION_ROTATE_PASSWORD:
      return "rotate_password";
    case CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY:
      return "replace_private_key";
    case CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE:
      return "remove_sensitive_value";
    case CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH:
      return "review_sensitive_hash";
    case CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER:
      return "review_password_verifier";
    case CRIBRA_REMEDIATION_UNKNOWN:
      return "unknown";
    default:
      return std::nullopt;
  }
}

inline std::optional<std::string_view> MapDetectionMode(uint32_t code) {
  switch (code) {
    case CRIBRA_DETECTION_MODE_NONE:
      return "none";
    case CRIBRA_DETECTION_MODE_MATCHER_ONLY:
      return "matcher_only";
    case CRIBRA_DETECTION_MODE_DETERMINISTIC:
      return "deterministic";
    case CRIBRA_DETECTION_MODE_CONTEXTUAL:
      return "contextual";
    case CRIBRA_DETECTION_MODE_UNKNOWN:
      return "unknown";
    default:
      return std::nullopt;
  }
}

inline std::optional<std::string_view> MapCandidateEvidence(uint32_t code) {
  switch (code) {
    case CRIBRA_CANDIDATE_EVIDENCE_NONE:
      return "none";
    case CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL:
      return "structural";
    case CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN:
      return "unknown";
    default:
      return std::nullopt;
  }
}

inline std::optional<std::string_view> MapCandidateKind(uint32_t code) {
  switch (code) {
    case CRIBRA_CANDIDATE_KIND_NONE:
      return "none";
    case CRIBRA_CANDIDATE_KIND_RECOVERY_LIKE_CODE:
      return "recovery_like_code";
    case CRIBRA_CANDIDATE_KIND_UNKNOWN:
      return "unknown";
    default:
      return std::nullopt;
  }
}

// Rule metadata uses the same Core matching families as the C ABI. There is no
// sentinel: an unrecognized kind fails the metadata projection closed.
inline std::optional<std::string_view> MapRuleKind(uint32_t code) {
  switch (code) {
    case CRIBRA_RULE_KIND_LITERAL:
      return "literal";
    case CRIBRA_RULE_KIND_PREFIX:
      return "prefix";
    case CRIBRA_RULE_KIND_SUFFIX:
      return "suffix";
    case CRIBRA_RULE_KIND_PATTERN:
      return "pattern";
    default:
      return std::nullopt;
  }
}

// An active rule always has a concrete detection authority. The NONE and
// UNKNOWN sentinels belong to explanation payloads, not rule metadata.
inline std::optional<std::string_view> MapRuleDetectionMode(uint32_t code) {
  switch (code) {
    case CRIBRA_DETECTION_MODE_MATCHER_ONLY:
      return "matcher_only";
    case CRIBRA_DETECTION_MODE_DETERMINISTIC:
      return "deterministic";
    case CRIBRA_DETECTION_MODE_CONTEXTUAL:
      return "contextual";
    default:
      return std::nullopt;
  }
}

// Input-direction mappings for the public configuration vocabulary. Unknown
// labels return std::nullopt so the bridge can reject them before Core work.
inline std::optional<uint32_t> MapRuleKindCode(std::string_view label) {
  if (label == "literal") {
    return CRIBRA_RULE_KIND_LITERAL;
  }
  if (label == "prefix") {
    return CRIBRA_RULE_KIND_PREFIX;
  }
  if (label == "suffix") {
    return CRIBRA_RULE_KIND_SUFFIX;
  }
  if (label == "pattern") {
    return CRIBRA_RULE_KIND_PATTERN;
  }
  return std::nullopt;
}

inline std::optional<uint32_t> MapSeverityCode(std::string_view label) {
  if (label == "info") {
    return CRIBRA_SEVERITY_INFO;
  }
  if (label == "low") {
    return CRIBRA_SEVERITY_LOW;
  }
  if (label == "medium") {
    return CRIBRA_SEVERITY_MEDIUM;
  }
  if (label == "high") {
    return CRIBRA_SEVERITY_HIGH;
  }
  if (label == "critical") {
    return CRIBRA_SEVERITY_CRITICAL;
  }
  return std::nullopt;
}

// An empty label is the configuration form of CRIBRA_REMEDIATION_NONE.
inline std::optional<uint32_t> MapRemediationCode(std::string_view label) {
  if (label.empty()) {
    return CRIBRA_REMEDIATION_NONE;
  }
  if (label == "revoke_and_rotate_credential") {
    return CRIBRA_REMEDIATION_REVOKE_AND_ROTATE_CREDENTIAL;
  }
  if (label == "rotate_credential") {
    return CRIBRA_REMEDIATION_ROTATE_CREDENTIAL;
  }
  if (label == "rotate_password") {
    return CRIBRA_REMEDIATION_ROTATE_PASSWORD;
  }
  if (label == "replace_private_key") {
    return CRIBRA_REMEDIATION_REPLACE_PRIVATE_KEY;
  }
  if (label == "remove_sensitive_value") {
    return CRIBRA_REMEDIATION_REMOVE_SENSITIVE_VALUE;
  }
  if (label == "review_sensitive_hash") {
    return CRIBRA_REMEDIATION_REVIEW_SENSITIVE_HASH;
  }
  if (label == "review_password_verifier") {
    return CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER;
  }
  // CRIBRA_REMEDIATION_UNKNOWN is an output sentinel only; Core rejects it as
  // an input remediation.
  return std::nullopt;
}

// Diagnostic names for the stable build-error categories. Used by the host
// protocol check to pin the numeric ABI contract.
inline std::optional<std::string_view> MapBuildErrorCode(uint32_t code) {
  switch (code) {
    case CRIBRA_BUILD_CODE_NONE:
      return "none";
    case CRIBRA_BUILD_CODE_INVALID_PATTERN:
      return "invalid_pattern";
    case CRIBRA_BUILD_CODE_PATTERN_MATCHES_EMPTY:
      return "pattern_matches_empty";
    case CRIBRA_BUILD_CODE_MISSING_CAPTURE_GROUP:
      return "missing_capture_group";
    case CRIBRA_BUILD_CODE_EMPTY_RULE_ID:
      return "empty_rule_id";
    case CRIBRA_BUILD_CODE_DUPLICATE_RULE_ID:
      return "duplicate_rule_id";
    case CRIBRA_BUILD_CODE_EMPTY_MATCHER:
      return "empty_matcher";
    case CRIBRA_BUILD_CODE_TOO_MANY_RULES:
      return "too_many_rules";
    case CRIBRA_BUILD_CODE_AUTOMATON_BUILD:
      return "automaton_build";
    default:
      return std::nullopt;
  }
}

/// True only for the configuration whose Core result is identical to
/// `Scanner::default()`: the canonical catalog and no other rule source.
constexpr bool IsCanonicalDefaultConfig(
    bool current,
    bool financial,
    bool personal,
    bool hasRules) noexcept {
  return current && !financial && !personal && !hasRules;
}

struct ExplanationMapping {
  std::string_view kind;
  std::string_view value;
};

// Combines an explanation kind with both payloads.
//
// The released C ABI emits:
//   - CLASSIFIED with a concrete detection mode and evidence NONE;
//   - AMBIGUOUS with detection mode NONE and concrete evidence;
//   - UNKNOWN with both payloads set to their UNKNOWN sentinels (the header's
//     documented zero-value form, both NONE, is accepted too).
//
// Anything else - an unrecognized code, a payload in the wrong context, a
// mixed UNKNOWN/NONE pair, or a classified/ambiguous kind with no concrete
// authority - is impossible and fails closed.
inline std::optional<ExplanationMapping> MapExplanation(
    uint32_t kind,
    uint32_t detectionMode,
    uint32_t candidateEvidence) {
  switch (kind) {
    case CRIBRA_EXPLANATION_NONE:
      if (detectionMode != CRIBRA_DETECTION_MODE_NONE ||
          candidateEvidence != CRIBRA_CANDIDATE_EVIDENCE_NONE) {
        return std::nullopt;
      }
      return ExplanationMapping{"none", ""};
    case CRIBRA_EXPLANATION_CLASSIFIED: {
      // A classification needs a concrete authority, and the ambiguous payload
      // must stay at its NONE sentinel.
      if (candidateEvidence != CRIBRA_CANDIDATE_EVIDENCE_NONE) {
        return std::nullopt;
      }
      if (detectionMode != CRIBRA_DETECTION_MODE_MATCHER_ONLY &&
          detectionMode != CRIBRA_DETECTION_MODE_DETERMINISTIC &&
          detectionMode != CRIBRA_DETECTION_MODE_CONTEXTUAL) {
        return std::nullopt;
      }
      return ExplanationMapping{
          "classified", MapDetectionMode(detectionMode).value_or("unknown")};
    }
    case CRIBRA_EXPLANATION_AMBIGUOUS: {
      // An ambiguity needs concrete evidence, and the classified payload must
      // stay at its NONE sentinel.
      if (detectionMode != CRIBRA_DETECTION_MODE_NONE) {
        return std::nullopt;
      }
      if (candidateEvidence != CRIBRA_CANDIDATE_EVIDENCE_STRUCTURAL &&
          candidateEvidence != CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN) {
        return std::nullopt;
      }
      return ExplanationMapping{
          "ambiguous", MapCandidateEvidence(candidateEvidence).value_or("unknown")};
    }
    case CRIBRA_EXPLANATION_UNKNOWN: {
      const bool allNone =
          detectionMode == CRIBRA_DETECTION_MODE_NONE &&
          candidateEvidence == CRIBRA_CANDIDATE_EVIDENCE_NONE;
      const bool allUnknown =
          detectionMode == CRIBRA_DETECTION_MODE_UNKNOWN &&
          candidateEvidence == CRIBRA_CANDIDATE_EVIDENCE_UNKNOWN;
      if (!allNone && !allUnknown) {
        return std::nullopt;
      }
      return ExplanationMapping{"unknown", ""};
    }
    default:
      return std::nullopt;
  }
}

}  // namespace cribra::rn
