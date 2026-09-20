import json
from pathlib import Path
import tempfile
import unittest

from tools.build_cli_reference import normalize, write_reference


def fixture() -> dict:
    return {
        "schema_version": "rustqec.cli.v1",
        "global_arguments": [{
            "name": "error_format", "flag": "--error-format", "required": False,
            "values": ["human", "json"], "default": None,
        }],
        "commands": [{
            "name": "circuit.stats",
            "argv": ["circuit", "stats"],
            "input_sources": ["stdin", "file"],
            "formats": ["human", "json"],
            "output_schema": "rustqec.cli.v1",
            "arguments": [{
                "name": "input", "flag": "--in", "required": False,
                "values": ["path"], "default": "stdin",
            }],
            "success_exit_code": 0,
            "errors": [{"code": "input_error", "exit_code": 2, "channel": "stderr"}],
        }],
    }


class BuildCliReferenceTest(unittest.TestCase):
    def test_normalizes_command_contract_for_templates(self):
        result = normalize(fixture())
        command = result["commands"][0]
        self.assertEqual(command["command_line"], "rustqec circuit stats")
        self.assertEqual(command["anchor"], "command-circuit-stats")
        self.assertEqual([entry["value"] for entry in result["exit_codes"]], [0, 2])
        self.assertEqual(result["exit_codes"][1]["errors"], ["input_error"])
        self.assertEqual(result["global_arguments"][0]["flag"], "--error-format")
        self.assertEqual(command["artifacts"], [])

    def test_rejects_duplicate_commands_and_flags(self):
        document = fixture()
        document["commands"].append(dict(document["commands"][0]))
        with self.assertRaisesRegex(ValueError, "unique"):
            normalize(document)

    def test_rejects_malformed_rendered_contract_fields(self):
        cases = []
        document = fixture()
        document["global_arguments"][0]["required"] = "no"
        cases.append((document, "required"))
        document = fixture()
        document["commands"][0]["output_schema"] = ""
        cases.append((document, "output_schema"))
        document = fixture()
        document["commands"][0]["errors"][0]["channel"] = "console"
        cases.append((document, "channel"))
        document = fixture()
        document["commands"][0]["artifacts"] = [{"name": "report", "flag": "--out"}]
        cases.append((document, "format"))
        for document, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                normalize(document)
        document = fixture()
        document["commands"][0]["arguments"].append(dict(document["commands"][0]["arguments"][0]))
        with self.assertRaisesRegex(ValueError, "unique"):
            normalize(document)

    def test_writes_deterministic_json(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "reference.json"
            write_reference(fixture(), output)
            value = json.loads(output.read_text())
            self.assertEqual(value["source_command"], "rustqec capabilities --format json")
            self.assertTrue(output.read_text().endswith("\n"))


if __name__ == "__main__":
    unittest.main()
