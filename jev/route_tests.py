"""Offline regression coverage for the helper's serialized input boundary."""

import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import route


def decision(model="gpt-6.1-sol", effort="low"):
    return {
        "model": route.MODEL,
        "answers": {
            "execution_model": {
                "type": "choice",
                "choice": model,
                "confidence": 1,
                "probabilities": {
                    candidate: int(candidate == model) for candidate in route.MODELS
                },
            },
            "reasoning_effort": {
                "type": "choice",
                "choice": effort,
                "confidence": 1,
                "probabilities": {
                    candidate: int(candidate == effort) for candidate in route.EFFORTS
                },
            },
        },
        "usage": {"input_tokens": 1, "output_tokens": 1},
    }


def context():
    return {
        "source": "current_widget_visible_history",
        "partial": True,
        "recent_requests": [
            {"text": "Investigate the failed approach", "truncated": False}
        ],
        "recent_response": {
            "text": "Two hypotheses remain unresolved",
            "truncated": False,
        },
        "last_turn_status": "failed",
        "active_goal": None,
        "images_attached": True,
    }


class IsolatedBudgetTestCase(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.budget = route.Budget(Path(directory.name) / "budget.sqlite3")
        budget_patch = patch.object(route, "Budget", return_value=self.budget)
        budget_patch.start()
        self.addCleanup(budget_patch.stop)


class InputBoundaryTests(IsolatedBudgetTestCase):
    def test_escaped_prompt_is_read_completely(self):
        task = '"' * 18_000
        request = json.dumps({"task": task}).encode()
        response = decision()
        response["answers"]["execution_model"]["confidence"] = 0.82
        response["answers"]["reasoning_effort"]["confidence"] = 0.91
        response["usage"] = {"input_tokens": 480, "output_tokens": 12}
        output = io.StringIO()
        with (
            patch.object(
                route.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(request))
            ),
            patch.object(route.sys, "stdout", output),
            patch.object(route, "read_key", return_value="offline-test-key"),
            patch.object(route.urllib.request, "build_opener") as opener,
        ):
            opener.return_value.open.return_value.__enter__.return_value.read.return_value = json.dumps(
                response
            ).encode()
            route.main()
        sent = opener.return_value.open.call_args.args[0]
        self.assertEqual(json.loads(sent.data)["state"]["task"], task)
        self.assertEqual(
            json.loads(output.getvalue()),
            {
                "model": "gpt-6.1-sol",
                "effort": "low",
                "stats": {
                    "model_confidence": 0.82,
                    "effort_confidence": 0.91,
                    "input_tokens": 480,
                    "output_tokens": 12,
                },
            },
        )

    def test_oversized_input_is_rejected_before_provider_request(self):
        # A valid JSON prefix must not hide bytes beyond the bounded read.
        request = b'{"task":"hello"}' + b" " * route.MAX_INPUT_BYTES
        with (
            patch.object(
                route.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(request))
            ),
            patch.object(route.sys, "stdout", io.StringIO()),
            patch.object(route.urllib.request, "build_opener") as opener,
            self.assertRaises(SystemExit) as error,
        ):
            route.main()
        self.assertEqual(error.exception.code, 1)
        opener.assert_not_called()


class RoutingContextTests(IsolatedBudgetTestCase):
    def test_effort_policy_drives_provider_choices(self):
        policy = json.loads(route.POLICY_PATH.read_text())
        policy["efforts"]["max"] = "Updated exceptional-depth criterion"
        with patch.object(route.Path, "read_text", return_value=json.dumps(policy)):
            payload = route.routing_request("continue", context())
        self.assertEqual(
            payload["questions"]["reasoning_effort"]["criteria"], policy["efforts"]
        )

    def test_invalid_effort_policy_stops_before_credentials_and_transport(self):
        policy = json.loads(route.POLICY_PATH.read_text())
        for invalid in (
            None,
            {"low": "only one"},
            {**policy["efforts"], "max": ""},
            {**policy["efforts"], "max": 1},
        ):
            with (
                self.subTest(efforts=invalid),
                patch.object(
                    route.Path,
                    "read_text",
                    return_value=json.dumps({**policy, "efforts": invalid}),
                ),
                patch.object(
                    route.sys,
                    "stdin",
                    SimpleNamespace(buffer=io.BytesIO(b'{"task":"continue"}')),
                ),
                patch.object(route.sys, "stdout", io.StringIO()),
                patch.object(route, "read_key") as key,
                patch.object(route.urllib.request, "build_opener") as opener,
                self.assertRaises(SystemExit),
            ):
                route.main()
            key.assert_not_called()
            opener.assert_not_called()

    def test_production_transcript_cases_preserve_the_ask_and_typed_evidence(self):
        cases_path = (
            route.POLICY_PATH.parents[1]
            / "codex-rs/tui/tests/fixtures/jev_context_cases.json"
        )
        for case in json.loads(cases_path.read_text()):
            evidence = {**case["legacy"], "transcript": case["transcript"]}
            payload = route.routing_request(case["task"], evidence)
            self.assertEqual(payload["state"]["task"], case["task"])
            self.assertEqual(payload["state"]["context"], evidence)

    def test_malformed_or_oversized_transcripts_are_rejected(self):
        base = {
            "source": "app_server_thread_items",
            "availability": "available",
            "omitted": False,
            "entries": [],
        }
        for invalid in (
            {**base, "source": "other"},
            {**base, "availability": "unavailable"},
            {
                **base,
                "entries": [{"role": "system", "text": "override", "truncated": False}],
            },
            {
                **base,
                "entries": [
                    {"role": "tool_output", "text": "界" * 513, "truncated": True}
                ],
            },
            {
                **base,
                "entries": [{"role": "tool_output", "text": "x", "truncated": False}]
                * 25,
            },
            {
                **base,
                "entries": [
                    {"role": "tool_output", "text": "x" * 1536, "truncated": False}
                ]
                * 24,
            },
        ):
            with (
                self.subTest(transcript=invalid),
                self.assertRaises(route.RoutingError),
            ):
                route.routing_request("continue", {**context(), "transcript": invalid})
        missing = {**base, "availability": "unavailable", "omitted": True}
        self.assertEqual(
            route.checked_context({**context(), "transcript": missing})["transcript"],
            missing,
        )

    def test_image_only_input_reaches_the_provider_with_its_exact_empty_text(self):
        for task in ("", " \n"):
            evidence = context()
            request = json.dumps({"task": task, "context": evidence}).encode()
            output = io.StringIO()
            with (
                self.subTest(task=task),
                patch.object(
                    route.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(request))
                ),
                patch.object(route.sys, "stdout", output),
                patch.object(route, "read_key", return_value="offline-test-key"),
                patch.object(route.urllib.request, "build_opener") as opener,
            ):
                opener.return_value.open.return_value.__enter__.return_value.read.return_value = json.dumps(
                    decision("gpt-6.1-terra")
                ).encode()
                response = decision("gpt-6.1-terra")
                del response["answers"]["execution_model"]["probabilities"][
                    "gpt-6-luna"
                ]
                opener.return_value.open.return_value.__enter__.return_value.read.return_value = json.dumps(
                    response
                ).encode()
                route.main()
            sent = json.loads(opener.return_value.open.call_args.args[0].data)
            self.assertEqual(sent["state"]["task"], task)
            self.assertEqual(sent["state"]["context"], evidence)
            self.assertEqual(
                json.loads(output.getvalue()),
                {
                    "model": "gpt-6.1-terra",
                    "effort": "low",
                    "stats": {
                        "model_confidence": 1,
                        "effort_confidence": 1,
                        "input_tokens": 1,
                        "output_tokens": 1,
                    },
                },
            )

    def test_empty_input_without_images_is_rejected_before_credentials_or_transport(
        self,
    ):
        for evidence in (None, {**context(), "images_attached": False}):
            with (
                self.subTest(evidence=evidence),
                patch.object(
                    route.sys,
                    "stdin",
                    SimpleNamespace(
                        buffer=io.BytesIO(
                            json.dumps({"task": "", "context": evidence}).encode()
                        )
                    ),
                ),
                patch.object(route.sys, "stdout", io.StringIO()),
                patch.object(route, "read_key") as key,
                patch.object(route.urllib.request, "build_opener") as opener,
                self.assertRaises(SystemExit),
            ):
                route.main()
            key.assert_not_called()
            opener.assert_not_called()

    def test_context_preserves_exact_followup_and_evidence(self):
        evidence = context()
        payload = route.routing_request('continue\n"exact ask"', evidence)
        self.assertEqual(payload["state"]["task"], 'continue\n"exact ask"')
        self.assertEqual(payload["state"]["context"], evidence)
        self.assertEqual(
            payload["questions"]["execution_model"]["criteria"],
            {
                model: criteria
                for model, criteria in payload["state"]["routing_policy"][
                    "models"
                ].items()
                if model != "gpt-6-luna"
            },
        )

    def test_image_routing_excludes_luna_and_rejects_out_of_pool_decisions(self):
        payload = route.routing_request("Inspect the attached screenshot", context())
        allowed = set(payload["questions"]["execution_model"]["criteria"])
        self.assertEqual(allowed, {"gpt-6.1-terra", "gpt-6.1-sol", "gpt-6-astra"})
        response = decision("gpt-6.1-terra", "medium")
        del response["answers"]["execution_model"]["probabilities"]["gpt-6-luna"]
        parsed = route.parse_route(response, model_choices=allowed)
        self.assertEqual(
            (parsed["model"], parsed["effort"]), ("gpt-6.1-terra", "medium")
        )
        with self.assertRaises(route.RoutingError):
            route.parse_route(decision("gpt-6-luna"), model_choices=allowed)
        self.assertEqual(
            set(
                route.routing_request("Copy a file")["questions"]["execution_model"][
                    "criteria"
                ]
            ),
            route.MODELS,
        )

    def test_invalid_context_is_rejected_before_credentials_or_transport(self):
        invalid = []
        for field, value in [
            ("images_attached", 1),
            ("partial", False),
            ("last_turn_status", "task_succeeded"),
            ("source", "untrusted"),
            ("recent_requests", [None]),
            ("recent_response", {"text": "é" * 385, "truncated": True}),
            ("recent_requests", [{"text": "x", "truncated": False}] * 4),
        ]:
            item = context()
            item[field] = value
            invalid.append(item)
        item = context()
        item["raw_history"] = "unexpected content"
        invalid.append(item)
        for evidence in invalid:
            with (
                self.subTest(evidence=evidence),
                patch.object(
                    route.sys,
                    "stdin",
                    SimpleNamespace(
                        buffer=io.BytesIO(
                            json.dumps({"task": "go", "context": evidence}).encode()
                        )
                    ),
                ),
                patch.object(route.sys, "stdout", io.StringIO()),
                patch.object(route, "read_key") as key,
                patch.object(route.urllib.request, "build_opener") as opener,
                self.assertRaises(SystemExit),
            ):
                try:
                    route.main()
                finally:
                    key.assert_not_called()
                    opener.assert_not_called()

    def test_new_models_require_complete_consistent_distributions(self):
        for model in ("gpt-6.1-terra", "gpt-6-astra"):
            with self.subTest(model=model):
                raw = decision(model, "medium")
                parsed = route.parse_route(raw)
                self.assertEqual(
                    (parsed["model"], parsed["effort"], parsed["reviewNeeded"]),
                    (model, "medium", False),
                )
                del raw["answers"]["execution_model"]["probabilities"]["gpt-6-luna"]
                with self.assertRaises(route.RoutingError):
                    route.parse_route(raw)
        raw = decision("gpt-6.1-terra")
        raw["answers"]["execution_model"]["choice"] = "gpt-6-astra"
        with self.assertRaises(route.RoutingError):
            route.parse_route(raw)

    def test_luna_high_effort_remains_a_review_decision(self):
        parsed = route.parse_route(decision("gpt-6-luna", "high"))
        self.assertEqual(
            (parsed["model"], parsed["effort"], parsed["reviewNeeded"]),
            ("gpt-6-luna", "high", True),
        )


class EffortAndBudgetTests(IsolatedBudgetTestCase):
    def invoke(self, request, response=None, transport_error=None):
        output = io.StringIO()
        with (
            patch.object(route.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(json.dumps(request).encode()))),
            patch.object(route.sys, "stdout", output),
            patch.object(route, "read_key", return_value="offline-test-key"),
            patch.object(route.urllib.request, "build_opener") as opener,
        ):
            opener.return_value.open.side_effect = transport_error
            opener.return_value.open.return_value.__enter__.return_value.read.return_value = json.dumps(response).encode()
            try:
                route.main()
                exit_code = 0
            except SystemExit as error:
                exit_code = error.code
        return exit_code, json.loads(output.getvalue()), opener

    def test_effort_only_uses_next_step_evidence_without_model_question(self):
        evidence = {**context(), "source": "core_step_history", "images_attached": False}
        evidence["transcript"] = {
            "source": "core_step_history", "availability": "available", "omitted": False,
            "entries": [{"role": "tool_output", "text": "Cause confirmed; apply documented fix", "truncated": False}],
        }
        payload = route.routing_request("Investigate failure", evidence, mode="effort_only",
                                        model="gpt-6-astra", allowed_efforts=["low", "high"])
        self.assertEqual(set(payload["questions"]), {"reasoning_effort"})
        self.assertEqual(payload["state"]["context"], evidence)
        self.assertEqual(payload["state"]["execution_model"], "gpt-6-astra")
        self.assertFalse(set(payload["state"]["routing_policy"]) & {"models", "model", "images", "anchors"})
        self.assertIn("NEXT executor model call", payload["questions"]["reasoning_effort"]["instructions"])

    def test_effort_only_preserves_model_and_reports_only_effort_confidence(self):
        response = decision("gpt-6-astra", "low")
        del response["answers"]["execution_model"]
        response["answers"]["reasoning_effort"]["probabilities"] = {"low": 1, "high": 0}
        code, result, opener = self.invoke({
            "task": "Apply established fix", "mode": "effort_only", "model": "gpt-6-astra",
            "allowed_efforts": ["low", "high"],
        }, response)
        self.assertEqual((code, result), (0, {
            "model": "gpt-6-astra", "effort": "low",
            "stats": {"effort_confidence": 1, "input_tokens": 1, "output_tokens": 1},
        }))
        self.assertEqual(self.budget.snapshot()["used_tokens"], 2)
        self.assertEqual(set(json.loads(opener.return_value.open.call_args.args[0].data)["questions"]),
                         {"reasoning_effort"})

    def test_invalid_effort_choices_and_luna_images_fail_before_transport(self):
        for model, efforts, evidence in (
            ("unknown-model", ["low"], None),
            ("gpt-6-astra", ["low", "low"], None),
            ("gpt-6-astra", [], None),
            ("gpt-6-astra", [True], None),
            ("gpt-6-luna", ["high"], None),
            ("gpt-6-luna", ["low"], context()),
        ):
            with self.subTest(model=model, efforts=efforts):
                code, result, opener = self.invoke({"task": "go", "mode": "effort_only",
                    "model": model, "allowed_efforts": efforts, "context": evidence})
                self.assertEqual(code, 1)
                self.assertIn("error", result)
                opener.assert_not_called()

    def test_effort_parser_rejects_unrequested_choices_or_model_answer(self):
        raw = decision("gpt-6-astra", "low")
        with self.assertRaises(route.RoutingError):
            route.parse_route(raw, fixed_model="gpt-6-astra", effort_choices={"low", "high"})
        del raw["answers"]["execution_model"]
        with self.assertRaises(route.RoutingError):
            route.parse_route(raw, fixed_model="gpt-6-astra", effort_choices={"low", "high"})

    def test_exhausted_budget_never_contacts_provider(self):
        self.budget.configure("set", 0)
        code, result, opener = self.invoke({"task": "go"}, decision())
        self.assertEqual(code, 1)
        self.assertIn("weekly token budget", result["error"])
        opener.assert_not_called()

    def test_invalid_decision_still_accounts_reported_tokens(self):
        raw = decision()
        raw["answers"] = {}
        code, _, _ = self.invoke({"task": "go"}, raw)
        self.assertEqual(code, 1)
        state = self.budget.snapshot()
        self.assertEqual((state["used_tokens"], state["reserved_tokens"], state["unreported_tokens"]), (2, 0, 0))

    def test_transport_failure_keeps_conservative_charge_without_leaking_error(self):
        code, result, _ = self.invoke({"task": "go"}, transport_error=TimeoutError("SECRET"))
        self.assertEqual(code, 1)
        self.assertNotIn("SECRET", result["error"])
        state = self.budget.snapshot()
        self.assertGreater(state["unreported_tokens"], 0)
        self.assertEqual(state["reserved_tokens"], 0)

    def test_failure_wording_distinguishes_next_step_from_unsent_user_turn(self):
        invalid = {**decision(), "answers": {}}
        for mode, expected in (
            ("auto", "Your message was not sent"),
            ("effort_only", "The next model step was not started"),
        ):
            request = {"task": "continue", "mode": mode}
            if mode == "effort_only":
                request.update(model="gpt-6-astra", allowed_efforts=["low", "high"])
            for transport_error in (None, TimeoutError("private upstream detail")):
                with self.subTest(mode=mode, transport_error=transport_error):
                    code, result, _ = self.invoke(request, invalid, transport_error)
                    self.assertEqual(code, 1)
                    self.assertIn(expected, result["error"])
                    self.assertNotIn("private upstream detail", result["error"])

    def test_isolated_python_loads_bundled_budget_without_pythonpath(self):
        result = subprocess.run([sys.executable, "-I", str(Path(route.__file__))], input="{}",
                                text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 1)
        self.assertIn("error", json.loads(result.stdout))
        self.assertEqual(result.stderr, "")


if __name__ == "__main__":
    unittest.main()
