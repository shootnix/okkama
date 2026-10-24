#!/usr/bin/env python3
"""Write schema/*.schema.json from the validator's constants (runner/ocs/model.py).

The executable validator is authoritative; these JSON Schema documents describe
the same formats for other tooling. runner-tests/test_schema_sync.py fails if
they drift apart.
"""
import json
import os
import sys

SUITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(SUITE, "runner"))
from ocs import model  # noqa: E402
from ocs.judge import ADAPTER_OUTCOMES  # noqa: E402

D = "https://json-schema.org/draft/2020-12/schema"
OUT = os.path.join(SUITE, "schema")


def output_spec():
    return {"oneOf": [
        {"const": "ignore"},
        {"type": "object", "required": ["text"], "properties": {"text": {"type": "string"}}},
        {"type": "object", "required": ["file"], "properties": {"file": {"type": "string"}}},
        {"type": "object", "required": ["bytes_b64"], "properties": {"bytes_b64": {"type": "string"}}},
        {"type": "object", "required": ["partial_order", "justification"], "properties": {
            "partial_order": {"type": "object", "required": ["lines"], "properties": {
                "lines": {"type": "array", "items": {"type": "string"}},
                "before": {"type": "array", "items": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 2}}}},
            "justification": {"type": "string", "minLength": 1}}}]}


def stage_spec(stages):
    return {"type": "object", "required": ["stages", "categories"], "additionalProperties": False, "properties": {
        "stages": {"type": "array", "minItems": 1, "items": {"enum": list(stages)}},
        "categories": {"type": "array", "minItems": 1, "items": {"enum": list(model.CATEGORIES)}},
        "error_type": {"type": "string"},
        "justification": {"type": "string"}}}


case = {
    "$schema": D, "$id": "ocs-case/1", "title": "Okkama conformance case (case.json)", "type": "object",
    "required": list(model.CASE_KEYS_REQUIRED),
    "additionalProperties": False,
    "properties": {
        "schema": {"const": model.CASE_SCHEMA},
        "id": {"type": "string", "pattern": model.ID_RE.pattern},
        "title": {"type": "string", "minLength": 1},
        "purpose": {"type": "string", "minLength": 1},
        "level": {"enum": sorted(model.LEVELS)},
        "contracts": {"type": "array", "minItems": 1, "items": {"type": "object", "required": ["contract", "aspects"],
                       "additionalProperties": False, "properties": {
                           "contract": {"type": "string", "pattern": model.CONTRACT_RE.pattern},
                           "aspects": {"type": "array", "minItems": 1, "items": {"enum": list(model.ASPECTS)}}}}},
        "mode": {"enum": list(model.MODES)},
        "sources": {"type": "array", "minItems": 1, "items": {"type": "string", "pattern": "\\.ok$"}},
        "entry": {"type": "string"},
        "support": {"type": "array", "items": {"type": "string"}},
        "args": {"type": "array", "items": {"type": "string"}},
        "stdin": {"type": "string"},
        "env": {"type": "object", "additionalProperties": {"type": "string"}},
        "requires": {"type": "object", "additionalProperties": False, "properties": {
            "environment": {"type": "array", "items": {"enum": list(model.ENVIRONMENT_TAGS)}},
            "fixtures": {"type": "array", "items": {"type": "string", "pattern": "^[a-z0-9-]+$"}}}},
        "harness": {"type": "boolean"},
        "expect": {"type": "object", "required": ["outcome"], "properties": {
            "outcome": {"enum": list(model.OUTCOMES)},
            "exit_status": {"type": "integer", "minimum": 0, "maximum": 255},
            "stdout": output_spec(), "stderr": output_spec(),
            "rejection": stage_spec(("parse", "check")),
            "failure": stage_spec(model.STAGES),
            "artifacts": {"type": "array", "items": {"type": "object", "required": ["path"]}},
            "justification": {"type": "string"}}},
        "spec_refs": {"type": "array", "minItems": 1, "items": {"type": "string"}},
        "budget": {"type": "object", "required": ["timeout_s"], "properties": {"timeout_s": {"type": "number", "exclusiveMinimum": 0, "maximum": 3600}}},
        "provenance": {"type": "object", "required": ["origin", "expected_basis"], "properties": {
            "origin": {"type": "string", "minLength": 1}, "expected_basis": {"type": "string", "minLength": 1}}},
        "status": {"enum": list(model.CASE_STATUSES)},
        "notes": {"type": "string"},
        "defects": {"type": "array", "items": {"type": "string", "pattern": "^DEF-[0-9]{3}$"}},
        "features": {"type": "array", "items": {"type": "string"}},
    }}
request = {"$schema": D, "$id": "ocs-adapter-request/1", "title": "Adapter request", "type": "object",
           "required": ["protocol", "mode", "sources", "args", "cwd", "stdlib_root", "timeout_s"], "additionalProperties": False,
           "properties": {"protocol": {"const": "ocs-adapter/1"}, "mode": {"enum": list(model.MODES)},
                          "sources": {"type": "array", "minItems": 1, "items": {"type": "string"}},
                          "args": {"type": "array", "items": {"type": "string"}}, "cwd": {"type": "string"},
                          "stdlib_root": {"type": "string"}, "timeout_s": {"type": "number"}}}
response = {"$schema": D, "$id": "ocs-adapter-response/1", "title": "Adapter response (observations only)", "type": "object",
            "required": ["protocol", "mode", "outcome"],
            "not": {"anyOf": [{"required": ["verdict"]}, {"required": ["pass"]}, {"required": ["status"]}]},
            "properties": {"protocol": {"const": "ocs-adapter/1"}, "mode": {"enum": list(model.MODES)},
                           "outcome": {"enum": list(ADAPTER_OUTCOMES)},
                           "exit_status": {"type": "integer", "minimum": 0, "maximum": 255},
                           "stage_reached": {"enum": ["none", "parse", "check", "runtime"]},
                           "unsupported_reason": {"type": "string"},
                           "failure": {"type": "object", "required": ["stage", "category"], "properties": {
                               "stage": {"enum": list(model.STAGES)},
                               "category": {"enum": list(model.CATEGORIES) + ["unclassified", "internal"]},
                               "error_type": {"type": "string"}, "message": {"type": "string"}, "raw": {"type": "string"}}},
                           "adapter": {"type": "object"}, "elapsed_ms": {"type": "integer"}}}
config = {"$schema": D, "$id": "ocs-adapter-config/1", "title": "Adapter configuration", "type": "object",
          "required": ["schema", "protocol", "name", "command"],
          "properties": {"schema": {"const": "ocs-adapter-config/1"}, "protocol": {"const": "ocs-adapter/1"},
                         "name": {"type": "string"}, "description": {"type": "string"},
                         "command": {"type": "array", "minItems": 1, "items": {"type": "string"}},
                         "env": {"type": "object", "additionalProperties": {"type": "string"}},
                         "pass_env": {"type": "array", "items": {"type": "string"}}, "notes": {"type": "string"}}}
profile = {"$schema": D, "$id": "ocs-profile/1", "title": "Profile", "type": "object",
           "required": ["schema", "name", "kind", "description", "select"],
           "properties": {"schema": {"const": model.PROFILE_SCHEMA}, "name": {"type": "string"},
                          "kind": {"enum": ["milestone", "release-member", "release-gate", "informative", "benchmark"]},
                          "description": {"type": "string"}, "members": {"type": "array", "items": {"type": "string"}},
                          "select": {"type": "object", "required": ["levels"], "additionalProperties": False, "properties": {
                              "levels": {"type": "array", "items": {"enum": sorted(model.LEVELS)}},
                              "modes": {"type": "array", "items": {"enum": list(model.MODES)}},
                              "environment_within": {"type": "array", "items": {"enum": list(model.ENVIRONMENT_TAGS)}},
                              "environment_any_of": {"type": "array", "items": {"enum": list(model.ENVIRONMENT_TAGS)}},
                              "mandatory_only": {"type": "boolean"},
                              "statuses": {"type": "array", "items": {"enum": list(model.CASE_STATUSES)}}}}}}
aspect = {"oneOf": [{"const": "required"}, {"type": "string", "pattern": "^n/a: .{4,}"}]}
contracts = {"$schema": D, "$id": "ocs-contracts/1", "title": "Contract inventory", "type": "object",
             "required": ["schema", "contracts"],
             "properties": {"schema": {"const": model.CONTRACTS_SCHEMA}, "family": {"type": "string"}, "description": {"type": "string"},
                            "contracts": {"type": "array", "items": {"type": "object", "required": ["id", "title", "status", "aspects", "spec_refs"],
                                "properties": {"id": {"type": "string", "pattern": model.CONTRACT_RE.pattern}, "title": {"type": "string"},
                                               "status": {"enum": ["mandatory", "unresolved", "deferred", "informative"]},
                                               "spec_refs": {"type": "array", "items": {"type": "string"}},
                                               "discrepancies": {"type": "array", "items": {"type": "string", "pattern": "^D-[0-9]{3}$"}},
                                               "aspects": {"type": "object", "required": list(model.ASPECTS),
                                                           "properties": {a: aspect for a in model.ASPECTS}}}}}}}
fixture = {"$schema": D, "$id": "ocs-fixture/1", "title": "Fixture", "type": "object", "required": ["schema", "kind"],
           "properties": {"schema": {"const": "ocs-fixture/1"}, "kind": {"enum": ["files", "service"]},
                          "path": {"type": "string"}, "target": {"type": "string"},
                          "command": {"type": "array", "items": {"type": "string"}}, "ready_regex": {"type": "string"},
                          "ready_timeout_s": {"type": "number"}, "provides": {"type": "array", "items": {"type": "string"}},
                          "description": {"type": "string"}}}
os.makedirs(OUT, exist_ok=True)
for name, doc in [("case", case), ("adapter-request", request), ("adapter-response", response), ("adapter-config", config),
                  ("profile", profile), ("contracts", contracts), ("fixture", fixture)]:
    with open(os.path.join(OUT, name + ".schema.json"), "w") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")
print("schemas written")
