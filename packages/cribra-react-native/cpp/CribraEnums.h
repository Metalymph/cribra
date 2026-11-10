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

struct ExplanationMapping {
  std::string_view kind;
  std::string_view value;
};

// Combines explanation kind with its payload. Sentinels are valid only in the
// context documented by CribraExplanationView: NONE/UNKNOWN mean absent/unknown
// metadata, while CLASSIFIED requires a detection mode and AMBIGUOUS requires
// candidate evidence. Impossible kind/payload combinations fail closed.
inline std::optional<ExplanationMapping> MapExplanation(
    uint32_t kind,
    uint32_t detectionMode,
    uint32_t candidateEvidence) {
  switch (kind) {
    case CRIBRA_EXPLANATION_NONE:
      return ExplanationMapping{"none", ""};
    case CRIBRA_EXPLANATION_CLASSIFIED: {
      const auto mode = MapDetectionMode(detectionMode);
      if (!mode) {
        return std::nullopt;
      }
      if (*mode == "none" || *mode == "unknown") {
        // A classified family without a concrete authority is not a
        // classification. Preserve it as documented unknown metadata.
        return ExplanationMapping{"unknown", ""};
      }
      return ExplanationMapping{"classified", *mode};
    }
    case CRIBRA_EXPLANATION_AMBIGUOUS: {
      const auto evidence = MapCandidateEvidence(candidateEvidence);
      if (!evidence) {
        return std::nullopt;
      }
      if (*evidence == "none") {
        return ExplanationMapping{"ambiguous", "unknown"};
      }
      return ExplanationMapping{"ambiguous", *evidence};
    }
    case CRIBRA_EXPLANATION_UNKNOWN:
      return ExplanationMapping{"unknown", ""};
    default:
      return std::nullopt;
  }
}

}  // namespace cribra::rn
