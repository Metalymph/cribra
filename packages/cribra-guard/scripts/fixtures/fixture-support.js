/**
 * Qualification fixture page support.
 *
 * This script runs in the page world, not the content script. It records
 * source-free counters and receipts so the qualification can prove whether a
 * fixture submission handler or endpoint ran. It never sends editor source.
 */
(function () {
  "use strict";

  var config = window.__fixtureConfig || {};
  var state = {
    submitHandlerCalls: 0,
    invalidCalls: 0,
    earlierWindowHandlerCalls: 0,
    autosaveCalls: 0,
    inputEvents: 0,
    changeEvents: 0,
    lastSubmitDefaultPrevented: null,
    receipts: []
  };
  window.__fixture = state;

  function record(kind, detail) {
    state.receipts.push({ kind: kind, detail: detail === undefined ? null : detail });
    try {
      fetch("/receipt", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          kind: kind,
          detail: detail === undefined ? null : detail
        }),
        keepalive: true
      }).catch(function () {});
    } catch (_) {
      // Qualification evidence also comes from the local server request log.
    }
  }

  function editorIn(form) {
    return form
      ? form.querySelector('[data-cribra-editor="source"]')
      : null;
  }

  function attachForm(form) {
    if (!form) return;
    form.addEventListener("submit", function (event) {
      state.submitHandlerCalls += 1;
      state.lastSubmitDefaultPrevented = event.defaultPrevented;
      var submitter = event.submitter;
      record(
        "submit-handler",
        submitter ? { id: submitter.id || null } : null
      );
    });
    form.addEventListener(
      "invalid",
      function () {
        state.invalidCalls += 1;
        record("invalid-handler", null);
      },
      true
    );
    var editor = editorIn(form);
    if (editor) {
      editor.addEventListener("input", function () {
        state.inputEvents += 1;
      });
      editor.addEventListener("change", function () {
        state.changeEvents += 1;
      });
    }
  }

  if (config.earlierWindowHandler) {
    window.addEventListener(
      "submit",
      function (event) {
        state.earlierWindowHandlerCalls += 1;
        record("earlier-window-handler", {
          defaultPrevented: event.defaultPrevented
        });
      },
      true
    );
  }

  attachForm(document.getElementById(config.formId || "fixture-form"));

  function currentForm() {
    return document.getElementById(config.formId || "fixture-form");
  }

  function editor() {
    return document.querySelector('[data-cribra-editor="source"]');
  }

  state.editorValue = function () {
    var element = editor();
    if (!element) return null;
    if (element.tagName === "TEXTAREA" || element.tagName === "INPUT") {
      return element.value;
    }
    return element.textContent || "";
  };

  state.formDataKeys = function () {
    var form = currentForm();
    if (!form) return [];
    try {
      var data = new FormData(form);
      var keys = [];
      data.forEach(function (_value, key) {
        keys.push(key);
      });
      return keys;
    } catch (_) {
      return [];
    }
  };

  state.setEditorValue = function (value, dispatchInput) {
    var element = editor();
    if (!element) return false;
    if (element.tagName === "TEXTAREA" || element.tagName === "INPUT") {
      element.value = value;
    } else {
      element.textContent = value;
    }
    if (dispatchInput) {
      element.dispatchEvent(
        new Event("input", { bubbles: true, composed: true })
      );
      element.dispatchEvent(new Event("change", { bubbles: true }));
    }
    return true;
  };

  state.replaceEditor = function () {
    var element = editor();
    if (!element) return false;
    var replacement = element.cloneNode(true);
    replacement.removeAttribute("id");
    if (
      replacement.tagName === "TEXTAREA" ||
      replacement.tagName === "INPUT"
    ) {
      replacement.value = "";
    } else {
      replacement.textContent = "";
    }
    element.replaceWith(replacement);
    attachForm(replacement.closest("form"));
    return true;
  };

  state.replaceForm = function (marked) {
    var form = currentForm();
    if (!form) return false;
    var replacement = form.cloneNode(true);
    var replacementEditor = editorIn(replacement);
    if (replacementEditor) {
      if (
        replacementEditor.tagName === "TEXTAREA" ||
        replacementEditor.tagName === "INPUT"
      ) {
        replacementEditor.value = "";
      } else {
        replacementEditor.textContent = "";
      }
    }
    if (!marked) replacement.removeAttribute("data-cribra-adapter");
    form.replaceWith(replacement);
    attachForm(replacement);
    return true;
  };

  state.dispatchNonCancellableSubmit = function () {
    var form = currentForm();
    if (!form) return false;
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: false })
    );
    return true;
  };

  state.callFormSubmit = function () {
    var form = currentForm();
    if (!form) return false;
    // A control with id="submit" shadows form.submit as a named property.
    HTMLFormElement.prototype.submit.call(form);
    return true;
  };

  state.requestSubmit = function (submitterId) {
    var form = currentForm();
    if (!form) return false;
    var submitter = submitterId
      ? document.getElementById(submitterId)
      : null;
    if (submitter) form.requestSubmit(submitter);
    else form.requestSubmit();
    return true;
  };

  state.directFetch = function () {
    record("direct-fetch", null);
    try {
      fetch("/direct").catch(function () {});
    } catch (_) {
      // Request-log evidence is authoritative.
    }
    return true;
  };

  state.autosave = function () {
    state.autosaveCalls += 1;
    record("autosave", null);
    try {
      fetch("/autosave").catch(function () {});
    } catch (_) {
      // Request-log evidence is authoritative.
    }
    return true;
  };

  state.resetCounters = function () {
    state.submitHandlerCalls = 0;
    state.invalidCalls = 0;
    state.earlierWindowHandlerCalls = 0;
    state.autosaveCalls = 0;
    state.inputEvents = 0;
    state.changeEvents = 0;
    state.lastSubmitDefaultPrevented = null;
    state.receipts.length = 0;
  };
})();
