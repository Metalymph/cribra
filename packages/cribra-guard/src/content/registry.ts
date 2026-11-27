/**
 * Adapter registration, detection, and coverage classification.
 *
 * The registry distinguishes:
 * - unrelated unsupported forms, which must remain untouched;
 * - explicitly recognized forms whose adapter is still supported;
 * - recognized forms that have become degraded or invalidated, which must not
 *   silently become an authorization bypass;
 * - previously protected forms that were replaced or lost, reported as a
 *   coverage loss rather than claimed as protected.
 */

import type {
  AdapterCoverage,
  AdapterIdentity,
  CoverageLossReport,
  PageAdapter,
  SupportedCoverage,
} from "./contract.js";
import { ContentEditableAdapter } from "./contenteditable-adapter.js";
import { NativeFormAdapter } from "./native-form-adapter.js";

export const REGISTRY_IDENTITY: AdapterIdentity = {
  id: "cribra.registry",
  version: "1.0.0",
};

const MAX_KNOWN_FORMS = 64;
const MAX_LOSS_REPORTS = 32;

export interface ProtectedRecord {
  readonly adapter: PageAdapter;
  readonly coverage: SupportedCoverage;
}

export interface RegistryOptions {
  readonly adapters?: readonly PageAdapter[];
  readonly now?: () => number;
}

function sameIdentity(left: AdapterIdentity, right: AdapterIdentity): boolean {
  return left.id === right.id && left.version === right.version;
}

export class AdapterRegistry {
  private readonly adapters: readonly PageAdapter[];
  private readonly known = new Map<HTMLFormElement, ProtectedRecord>();
  /**
   * Weak memory of every form that was ever supported. The bounded `known` map
   * can evict a connected form; this weak map ensures an evicted path is still
   * recognized and blocked instead of silently becoming unrelated.
   */
  private everKnown = new WeakMap<HTMLFormElement, AdapterIdentity>();
  private readonly lossReports: CoverageLossReport[] = [];
  private readonly lossKeys = new Set<string>();
  private readonly now: () => number;
  private disposed = false;

  constructor(options: RegistryOptions = {}) {
    this.adapters =
      options.adapters ?? [
        new NativeFormAdapter(),
        new ContentEditableAdapter(),
      ];
    this.now = options.now ?? (() => Date.now());
  }

  /** Synchronous classification for the current DOM. */
  detect(form: HTMLFormElement): AdapterCoverage {
    if (this.disposed) {
      return {
        status: "unsupported",
        identity: REGISTRY_IDENTITY,
        reason: "adapter_disposed",
      };
    }

    const adapter = this.adapters.find((candidate) => candidate.recognizes(form));
    if (adapter === undefined) {
      const previous = this.known.get(form);
      if (previous !== undefined) {
        // The marker itself was removed or the form no longer matches any
        // adapter. A previously protected path must not silently pass through.
        return {
          status: "invalidated",
          identity: previous.adapter.identity,
          reason: "adapter_invalidated",
          cause: "unrecognized_form",
        };
      }
      const everIdentity = this.everKnown.get(form);
      if (everIdentity !== undefined) {
        return {
          status: "invalidated",
          identity: everIdentity,
          reason: "adapter_invalidated",
          cause: "unrecognized_form",
        };
      }
      return {
        status: "unsupported",
        identity: REGISTRY_IDENTITY,
        reason: "unrecognized_form",
      };
    }

    const coverage = adapter.detect(form);
    const previous = this.known.get(form);

    if (coverage.status === "supported") {
      if (previous !== undefined) {
        if (previous.coverage.editor.token !== coverage.editor.token) {
          // A replaced editor is never silently re-authorized. The old
          // snapshot identity is gone; keep the original record and block.
          return {
            status: "invalidated",
            identity: adapter.identity,
            reason: "editor_replaced",
            cause: "editor_replaced",
          };
        }
        if (!sameIdentity(previous.adapter.identity, adapter.identity)) {
          return {
            status: "invalidated",
            identity: adapter.identity,
            reason: "adapter_changed",
            cause: "adapter_changed",
          };
        }
      } else if (this.everKnown.has(form)) {
        // The bounded known map evicted a still-connected protected form. Do
        // not silently start a new authorization path for it.
        return {
          status: "invalidated",
          identity: adapter.identity,
          reason: "adapter_invalidated",
          cause: "adapter_invalidated",
        };
      } else {
        this.known.set(form, { adapter, coverage });
        this.everKnown.set(form, adapter.identity);
        this.enforceBounds();
      }
      return coverage;
    }

    if (previous !== undefined) {
      if (coverage.status === "degraded") {
        return coverage;
      }
      if (coverage.status === "invalidated") {
        return {
          status: "invalidated",
          identity: adapter.identity,
          reason: coverage.reason,
          cause: coverage.reason,
        };
      }
      // A previously supported form whose designated editor vanished or became
      // unrecognized is still a known protected path. It must not fall through.
      return {
        status: "invalidated",
        identity: adapter.identity,
        reason: "adapter_invalidated",
        cause: coverage.reason,
      };
    }

    if (this.everKnown.has(form)) {
      if (coverage.status === "degraded") return coverage;
      return {
        status: "invalidated",
        identity: adapter.identity,
        reason: "adapter_invalidated",
        cause: coverage.reason,
      };
    }

    return coverage;
  }

  isKnown(form: HTMLFormElement): boolean {
    return this.known.has(form) || this.everKnown.has(form);
  }

  /** Identity of a known form, even if the bounded record was evicted. */
  identityFor(form: HTMLFormElement): AdapterIdentity | undefined {
    return this.known.get(form)?.adapter.identity ?? this.everKnown.get(form);
  }

  protectedRecords(): readonly ProtectedRecord[] {
    return Array.from(this.known.values());
  }

  /** Records one diagnostic loss, deduplicated by form and reason. */
  recordLoss(report: CoverageLossReport): boolean {
    const key = `${report.formToken}:${report.reason}`;
    if (this.lossKeys.has(key)) return false;
    this.lossKeys.add(key);
    this.lossReports.push(report);
    while (this.lossReports.length > MAX_LOSS_REPORTS) {
      this.lossReports.shift();
    }
    return true;
  }

  losses(): readonly CoverageLossReport[] {
    return [...this.lossReports];
  }

  forget(form: HTMLFormElement): void {
    this.known.delete(form);
  }

  dispose(): void {
    this.known.clear();
    this.everKnown = new WeakMap();
    this.lossReports.length = 0;
    this.lossKeys.clear();
    this.disposed = true;
  }

  /** Builds a loss report for a protected record. */
  lossFor(
    record: ProtectedRecord,
    reason: CoverageLossReport["reason"],
  ): CoverageLossReport {
    return {
      kind: "coverage_lost",
      identity: record.coverage.identity,
      formToken: record.coverage.formToken,
      reason,
      at: this.now(),
    };
  }

  private enforceBounds(): void {
    while (this.known.size > MAX_KNOWN_FORMS) {
      const entries = Array.from(this.known.entries());
      const removable =
        entries.find(([form]) => form.isConnected !== true) ?? entries[0];
      if (removable === undefined) return;
      const [form, record] = removable;
      this.known.delete(form);
      if (form.isConnected === true) {
        this.recordLoss(this.lossFor(record, "adapter_invalidated"));
      }
    }
  }
}
