#!/usr/bin/env python3
"""Unit tests for the pull request body check."""

import json
import os
import re
import subprocess
import sys
import tempfile
import textwrap
import threading
import unittest
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pr_body_check as check

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).resolve().parent / "pr_body_check.py"
CI = ROOT / ".github" / "workflows" / "ci.yml"
ORG = "groundinglab"
REPO = "pcoc"
HOST = "github.com"
PR = 5


def verdict_url(number=PR, comment="100", kind="pull", org=ORG, repo=REPO, host=HOST, frag=None):
    base = f"https://{host}/{org}/{repo}/{kind}/{number}"
    if frag is None:
        return base + "#issuecomment-" + comment
    if frag == "":
        return base
    return base + "#" + frag


def high_body(*lines):
    return "\n".join(["Risk tier: High", *lines]) + "\n"


class FakeTransport:
    def __init__(self, comments=None, linked=None, closing_error=None):
        self.comments = comments or {}
        self.linked = linked
        self.closing_error = closing_error
        self.comment_calls = []
        self.closing_calls = 0

    def comment(self, org, repo, comment_id):
        self.comment_calls.append(comment_id)
        if comment_id not in self.comments:
            raise check.CommentMissing()
        row = self.comments[comment_id]
        if row is None:
            raise check.CommentMissing()
        if isinstance(row, check.ApiError):
            raise row
        return row

    def closing_issues(self, org, repo, number):
        self.closing_calls += 1
        if self.closing_error is not None:
            raise self.closing_error
        if self.linked is None:
            raise AssertionError("closing_issues was called")
        return set(self.linked)


class TierTests(unittest.TestCase):
    def test_present_forms(self):
        cases = {
            "Risk tier: Low": "low",
            "Risk tier: Medium": "medium",
            "Risk tier: High": "high",
            "Risk tier: SPEC / L5-shaped": "spec / l5-shaped",
            "RISK TIER: low": "low",
            "Risk Tier: MeDiUm": "medium",
            "spec / l5-shaped\nRisk tier: SPEC / L5-shaped": "spec / l5-shaped",
            "Risk tier:   SPEC   /   L5-shaped": "spec / l5-shaped",
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                tier, err = check.parse_tier(body)
                self.assertIsNone(err)
                self.assertEqual(tier, want)

    def test_bold_forms(self):
        cases = {
            "**Risk tier:** High": "high",
            "**Risk tier: Low**": "low",
            "Risk tier: **Medium**": "medium",
            "**Risk tier:** **SPEC / L5-shaped**": "spec / l5-shaped",
            "__Risk tier:__ Low": "low",
            "__Risk tier:__ **High**": "high",
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                tier, err = check.parse_tier(body)
                self.assertIsNone(err)
                self.assertEqual(tier, want)

    def test_missing(self):
        for body in ("", "no tier here", "The risk tier is High", "Risk tier is High", "> Risk tier: Low"):
            with self.subTest(body=body):
                tier, err = check.parse_tier(body)
                self.assertIsNone(tier)
                self.assertEqual(err, check.MISSING_TIER)

    def test_misspelled(self):
        for body in (
            "Risk tier: Critical",
            "Risk tier: L5",
            "Risk tier: spec/l5-shaped",
            "Risk tier:",
            "Risk tier: high-ish",
        ):
            with self.subTest(body=body):
                tier, err = check.parse_tier(body)
                self.assertIsNone(tier)
                self.assertEqual(err, check.BAD_TIER)
                self.assertIn("Risk tier:", err)

    def test_disagreeing_lines(self):
        _tier, err = check.parse_tier("Risk tier: Low\nRisk tier: High\n")
        self.assertEqual(err, check.BAD_TIER)


class VerdictParserTests(unittest.TestCase):
    def details(self, body, allowed):
        rows = check.parse_verdicts(body, ORG, REPO, HOST, allowed)
        self.assertEqual([row.label for row in rows], ["Red Hat", "White Hat"])
        return rows

    def test_valid_lines_both_labels(self):
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="1"),
            "White Hat verdict: " + verdict_url(comment="2", kind="issues"),
        )
        rows = self.details(body, {PR})
        self.assertEqual([(row.status, row.detail, row.comment_id) for row in rows], [
            ("ok", "ok", "1"),
            ("ok", "ok", "2"),
        ])

    def test_markdown_link_and_bold(self):
        url = verdict_url(comment="3")
        body = high_body(
            "**Red Hat verdict:** [the comment](" + url + ")",
            "__White Hat verdict:__ **" + url + "**",
        )
        rows = self.details(body, {PR})
        self.assertTrue(all(row.status == "ok" and row.comment_id == "3" for row in rows))

    def test_upper_and_lower_case(self):
        url = verdict_url(comment="4")
        body = "\n".join([
            "RISK TIER: HIGH",
            "RED HAT VERDICT: " + url,
            "white hat verdict: " + url,
        ])
        rows = self.details(body, {PR})
        self.assertTrue(all(row.status == "ok" for row in rows))

    def test_case_insensitive_host_org_repo(self):
        url = verdict_url(comment="4", org="GroundingLab", repo="PCOC", host="GitHub.com")
        body = high_body(
            "Red Hat verdict: " + url,
            "White Hat verdict: " + url,
        )
        rows = self.details(body, {PR})
        self.assertTrue(all(row.detail == "ok" for row in rows))

    def test_wrong_label(self):
        body = high_body("Green Hat verdict: " + verdict_url())
        rows = self.details(body, {PR})
        self.assertTrue(all(row.status == "missing" for row in rows))

    def test_wrong_host(self):
        body = high_body(
            "Red Hat verdict: " + verdict_url(host="example.com"),
            "White Hat verdict: " + verdict_url(),
        )
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "wrong-host")
        self.assertEqual(rows[0].status, "malformed")
        self.assertEqual(rows[1].status, "ok")

    def test_wrong_org(self):
        body = high_body("Red Hat verdict: " + verdict_url(org="other"))
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "wrong-org")

    def test_wrong_repo(self):
        body = high_body("Red Hat verdict: " + verdict_url(repo="other"))
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "wrong-repo")

    def test_wrong_number(self):
        body = high_body("Red Hat verdict: " + verdict_url(number=99))
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "wrong-number")
        self.assertEqual(rows[0].status, "malformed")

    def test_linked_issue_number_is_allowed(self):
        body = high_body(
            "Red Hat verdict: " + verdict_url(number=160, kind="issues", comment="8"),
            "White Hat verdict: " + verdict_url(),
        )
        denied = self.details(body, {PR})
        self.assertEqual(denied[0].detail, "wrong-number")
        allowed = self.details(body, {PR, 160})
        self.assertEqual(allowed[0].detail, "ok")
        self.assertEqual(allowed[0].number, 160)

    def test_missing_anchor(self):
        body = high_body("Red Hat verdict: " + verdict_url(frag=""))
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "missing-anchor")

    def test_pullrequestreview_link(self):
        body = high_body("Red Hat verdict: " + verdict_url(frag="pullrequestreview-9"))
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "review-link")
        self.assertEqual(rows[0].status, "malformed")

    def test_extra_url(self):
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="1") + " " + verdict_url(comment="2"),
        )
        rows = self.details(body, {PR})
        self.assertEqual(rows[0].detail, "extra-url")

    def test_messages_name_the_verdict_and_do_not_echo_the_url(self):
        secret = verdict_url(host="example.com", comment="55")
        body = high_body("Red Hat verdict: " + secret)
        errors = check.run_check(body, ORG, REPO, PR, HOST, FakeTransport(), set())
        self.assertEqual(errors, [
            check.malformed_verdict("Red Hat"),
            check.missing_verdict("White Hat"),
        ])
        blob = "\n".join(errors)
        self.assertNotIn(secret, blob)
        self.assertNotIn("example.com", blob)
        self.assertNotIn("\n", errors[0])


class ApiTests(unittest.TestCase):
    def record(self, number):
        return check.CommentRecord(ORG, REPO, number)

    def test_low_and_medium_do_not_need_verdicts_or_the_api(self):
        transport = FakeTransport()
        for body in ("Risk tier: Low\n", "Risk tier: Medium\n", "**Risk tier:** Low\n$(id)\n"):
            errors = check.run_check(body, ORG, REPO, PR, HOST, transport, None)
            self.assertEqual(errors, [])
        self.assertEqual(transport.comment_calls, [])
        self.assertEqual(transport.closing_calls, 0)

    def test_high_and_spec_missing_verdicts(self):
        transport = FakeTransport()
        for tier in ("High", "SPEC / L5-shaped", "spec / l5-shaped"):
            errors = check.run_check(
                "Risk tier: " + tier + "\n", ORG, REPO, PR, HOST, transport, set()
            )
            self.assertEqual(errors, [
                check.missing_verdict("Red Hat"),
                check.missing_verdict("White Hat"),
            ])
        self.assertEqual(transport.comment_calls, [])

    def test_exists(self):
        transport = FakeTransport({
            "100": self.record(PR),
            "101": self.record(PR),
        })
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="100"),
            "White Hat verdict: " + verdict_url(comment="101"),
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, set())
        self.assertEqual(errors, [])
        self.assertEqual(transport.comment_calls, ["100", "101"])
        self.assertEqual(transport.closing_calls, 0)

    def test_deleted_404(self):
        transport = FakeTransport({"100": self.record(PR), "101": None})
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="100"),
            "White Hat verdict: " + verdict_url(comment="101"),
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, set())
        self.assertEqual(errors, [check.comment_not_found("White Hat")])

    def test_wrong_issue(self):
        transport = FakeTransport({"100": self.record(PR), "101": self.record(999)})
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="100"),
            "White Hat verdict: " + verdict_url(comment="101"),
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, set())
        self.assertEqual(errors, [check.comment_wrong_issue("White Hat")])
        self.assertNotIn("999", errors[0])

    def test_wrong_issue_org(self):
        transport = FakeTransport({
            "100": self.record(PR),
            "101": check.CommentRecord("other", REPO, PR),
        })
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="100"),
            "White Hat verdict: " + verdict_url(comment="101"),
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, set())
        self.assertEqual(errors, [check.comment_wrong_issue("White Hat")])

    def test_linked_issue_comment_exists(self):
        transport = FakeTransport(
            {"8": self.record(160), "100": self.record(PR)},
            linked={160},
        )
        body = high_body(
            "Red Hat verdict: " + verdict_url(number=160, kind="issues", comment="8"),
            "White Hat verdict: " + verdict_url(comment="100"),
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, None)
        self.assertEqual(errors, [])
        self.assertEqual(transport.closing_calls, 1)
        self.assertEqual(transport.comment_calls, ["8", "100"])

    def test_closing_refs_unread_does_not_widen(self):
        transport = FakeTransport(
            {"100": self.record(PR)},
            closing_error=check.ApiError("down"),
        )
        body = high_body(
            "Red Hat verdict: " + verdict_url(number=160, kind="issues", comment="8"),
            "White Hat verdict: " + verdict_url(comment="100"),
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, None)
        self.assertEqual(errors, [check.CLOSING_REFS_UNREAD])
        self.assertEqual(transport.comment_calls, ["100"])
        self.assertNotIn("8", transport.comment_calls)

    def test_missing_token(self):
        transport = FakeTransport()
        transport.comment = lambda org, repo, comment_id: (_ for _ in ()).throw(check.NoToken())
        body = high_body(
            "Red Hat verdict: " + verdict_url(comment="100"),
            "White Hat verdict: nope",
        )
        errors = check.run_check(body, ORG, REPO, PR, HOST, transport, set())
        self.assertEqual(errors, [
            check.malformed_verdict("White Hat"),
            check.NO_TOKEN,
        ])

    def test_injection_text_is_not_a_message(self):
        body = "Risk tier: Low\n$(touch /tmp/pcoc-160-pr-body-inject)\n`id`\n${{ secrets.G1_PATTERNS }}\n"
        errors = check.run_check(body, ORG, REPO, PR, HOST, FakeTransport(), None)
        self.assertEqual(errors, [])


class CliTests(unittest.TestCase):
    def run_cli(self, body, extra=(), comments=None, number=PR, event=None, env_extra=None):
        with tempfile.TemporaryDirectory() as tmp:
            event_path = os.path.join(tmp, "event.json")
            if event is None:
                event = {
                    "repository": {
                        "name": REPO,
                        "html_url": f"https://github.com/{ORG}/{REPO}",
                        "owner": {"login": ORG},
                    },
                    "pull_request": {"number": number, "body": body},
                }
            with open(event_path, "w", encoding="utf-8") as handle:
                json.dump(event, handle)
            args = [sys.executable, str(SCRIPT), "--event", event_path, *extra]
            if comments is not None:
                path = os.path.join(tmp, "comments.json")
                with open(path, "w", encoding="utf-8") as handle:
                    json.dump(comments, handle)
                args.extend(["--comments-file", path])
            env = os.environ.copy()
            env.pop("GITHUB_TOKEN", None)
            env.pop("GITHUB_EVENT_PATH", None)
            env["GITHUB_API_URL"] = "https://api.github.com"
            if env_extra:
                env.update(env_extra)
            return subprocess.run(
                args, capture_output=True, text=True, env=env, cwd=ROOT, check=False
            )

    def test_good_low(self):
        proc = self.run_cli("Risk tier: Low\n")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, check.OK + "\n")
        self.assertEqual(proc.stderr, "")

    def test_missing_tier(self):
        proc = self.run_cli("No tier in this body.\n")
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, "::error::" + check.MISSING_TIER + "\n")

    def test_misspelled_tier(self):
        proc = self.run_cli("Risk tier: Critical\n")
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, "::error::" + check.BAD_TIER + "\n")

    def test_bold_tier(self):
        proc = self.run_cli("**Risk tier:** Low\n")
        self.assertEqual(proc.returncode, 0, proc.stdout)
        self.assertEqual(proc.stdout, check.OK + "\n")

    def test_high_missing_verdicts(self):
        proc = self.run_cli("Risk tier: High\n")
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, "".join(
            "::error::" + line + "\n"
            for line in (
                check.missing_verdict("Red Hat"),
                check.missing_verdict("White Hat"),
            )
        ))

    def test_high_malformed_and_wrong_pr(self):
        body = high_body(
            "Red Hat verdict: " + verdict_url(frag=""),
            "White Hat verdict: " + verdict_url(number=99),
        )
        proc = self.run_cli(body, extra=("--linked-issues", ""))
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, "".join(
            "::error::" + check.malformed_verdict(label) + "\n"
            for label in ("Red Hat", "White Hat")
        ))
        self.assertNotIn("github.com", proc.stdout)

    def test_placeholder_pending_is_malformed(self):
        body = "\n".join([
            "Risk tier: High",
            "Red Hat verdict: (pending)",
            "White Hat verdict: (pending)",
        ]) + "\n"
        proc = self.run_cli(body)
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, "".join(
            "::error::" + check.malformed_verdict(label) + "\n"
            for label in ("Red Hat", "White Hat")
        ))

    def test_injection_body_does_not_execute(self):
        marker = "/tmp/pcoc-160-pr-body-inject"
        try:
            os.remove(marker)
        except FileNotFoundError:
            pass
        body = "\n".join([
            "Risk tier: Low",
            "$(touch " + marker + ")",
            "`touch " + marker + "`",
            "${{ secrets.G1_PATTERNS }}",
            "${{ github.event.pull_request.body }}",
        ]) + "\n"
        proc = self.run_cli(body)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, check.OK + "\n")
        self.assertFalse(os.path.exists(marker))
        self.assertNotIn("G1_PATTERNS", proc.stdout)
        self.assertNotIn("touch", proc.stdout)

    def test_comment_exists(self):
        url_api = "https://api.github.com/repos/groundinglab/pcoc/issues/5"
        proc = self.run_cli(
            high_body(
                "Red Hat verdict: " + verdict_url(comment="10"),
                "White Hat verdict: [note](" + verdict_url(comment="11") + ")",
            ),
            comments={
                "10": {"issue_url": url_api},
                "11": {"issue_url": url_api},
            },
        )
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(proc.stdout, check.OK + "\n")

    def test_comment_404(self):
        url_api = "https://api.github.com/repos/groundinglab/pcoc/issues/5"
        proc = self.run_cli(
            high_body(
                "Red Hat verdict: " + verdict_url(comment="10"),
                "White Hat verdict: " + verdict_url(comment="11"),
            ),
            comments={"10": {"issue_url": url_api}, "11": None},
        )
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(proc.stdout, "::error::" + check.comment_not_found("White Hat") + "\n")

    def test_comment_wrong_issue(self):
        proc = self.run_cli(
            high_body(
                "Red Hat verdict: " + verdict_url(comment="10"),
                "White Hat verdict: " + verdict_url(comment="12"),
            ),
            comments={
                "10": {"issue_url": "https://api.github.com/repos/groundinglab/pcoc/issues/5"},
                "12": {"issue_url": "https://api.github.com/repos/groundinglab/pcoc/issues/8"},
            },
        )
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(
            proc.stdout, "::error::" + check.comment_wrong_issue("White Hat") + "\n"
        )
        self.assertNotIn("/issues/8", proc.stdout)

    def test_push_event_does_not_fail(self):
        proc = self.run_cli(None, event={"ref": "refs/heads/main"})
        self.assertEqual(proc.returncode, 0)
        self.assertEqual(proc.stdout, check.NOT_A_PULL_REQUEST + "\n")

    def test_repository_from_env(self):
        event = {"repository": {}, "pull_request": {"number": 5, "body": "Risk tier: Low"}}
        proc = self.run_cli(None, event=event, env_extra={"GITHUB_REPOSITORY": "groundinglab/pcoc"})
        self.assertEqual(proc.returncode, 0, proc.stdout)
        self.assertEqual(proc.stdout, check.OK + "\n")


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.handler = QuietHandler
        self.handler.paths = []
        self.handler.auth = None
        self.handler.linked = [160]
        self.handler.mode = "ok"
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), self.handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        host, port = self.server.server_address
        self.api = f"http://{host}:{port}"
        self.handler.api = self.api
        self.transport = check.GitHubTransport(self.api, "test-token")

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()

    def test_exists_404_and_wrong_issue(self):
        found = self.transport.comment("acme", "widgets", "10")
        self.assertEqual((found.org, found.repo, found.number), ("acme", "widgets", 5))
        with self.assertRaises(check.CommentMissing):
            self.transport.comment("acme", "widgets", "11")
        other = self.transport.comment("acme", "widgets", "12")
        self.assertEqual(other.number, 8)
        self.assertEqual(self.handler.auth, "Bearer test-token")
        self.assertIn("/repos/acme/widgets/issues/comments/10", self.handler.paths)
        self.assertTrue(all(path.startswith("/repos/acme/widgets/") for path in self.handler.paths))

    def test_redirect_is_not_followed(self):
        with self.assertRaises(check.ApiError):
            self.transport.comment("acme", "widgets", "14")
        self.assertTrue(all("example.invalid" not in path for path in self.handler.paths))

    def test_closing_issues_and_comment_on_linked_issue(self):
        self.assertEqual(self.transport.closing_issues("acme", "widgets", 5), {160})
        body = "\n".join([
            "Risk tier: High",
            "Red Hat verdict: https://github.com/acme/widgets/issues/160#issuecomment-13",
            "White Hat verdict: https://github.com/acme/widgets/pull/5#issuecomment-10",
        ])
        errors = check.run_check(body, "acme", "widgets", 5, "github.com", self.transport, None)
        self.assertEqual(errors, [])

    def test_closing_issues_failure_is_fixed(self):
        self.handler.mode = "status-500"
        body = "\n".join([
            "Risk tier: SPEC / L5-shaped",
            "Red Hat verdict: https://github.com/acme/widgets/issues/160#issuecomment-13",
            "White Hat verdict: https://github.com/acme/widgets/pull/5#issuecomment-10",
        ])
        errors = check.run_check(body, "acme", "widgets", 5, "github.com", self.transport, None)
        self.assertEqual(errors, [check.CLOSING_REFS_UNREAD])

    def test_closing_issues_next_page_fails_closed(self):
        self.handler.mode = "next"
        with self.assertRaises(check.ApiError):
            self.transport.closing_issues("acme", "widgets", 5)

    def test_empty_token(self):
        transport = check.GitHubTransport(self.api, "")
        with self.assertRaises(check.NoToken):
            transport.comment("acme", "widgets", "10")


class QuietHandler(BaseHTTPRequestHandler):
    api = ""
    paths = []
    auth = None
    linked = [160]
    mode = "ok"

    def log_message(self, fmt, *args):
        return

    def _json(self, code, payload):
        raw = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        QuietHandler.paths.append(self.path)
        QuietHandler.auth = self.headers.get("Authorization")
        issue = {
            "10": 5,
            "12": 8,
            "13": 160,
        }
        name = self.path.rstrip("/").rsplit("/", 1)[-1]
        if name == "11":
            self._json(404, {"message": "Not Found"})
            return
        if name == "14":
            self.send_response(302)
            self.send_header("Location", "https://example.invalid/stolen")
            self.end_headers()
            return
        if name in issue:
            self._json(200, {
                "id": int(name),
                "issue_url": QuietHandler.api + "/repos/acme/widgets/issues/" + str(issue[name]),
            })
            return
        self._json(500, {})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        QuietHandler.paths.append(self.path)
        if QuietHandler.mode == "status-500":
            self._json(500, {"message": "no"})
            return
        if QuietHandler.mode == "next":
            nodes = [{"number": number} for number in QuietHandler.linked]
            page = True
        else:
            nodes = [{"number": number} for number in QuietHandler.linked]
            page = False
        self._json(200, {
            "data": {
                "repository": {
                    "pullRequest": {
                        "closingIssuesReferences": {
                            "pageInfo": {"hasNextPage": page},
                            "nodes": nodes,
                        }
                    }
                }
            }
        })


class WorkflowTests(unittest.TestCase):
    def text(self):
        return CI.read_text(encoding="utf-8")

    def job(self, name):
        text = self.text()
        marker = f"\n  {name}:\n"
        start = text.index(marker) + 1
        match = re.search(r"\n  [A-Za-z0-9_-]+:\n", text[start + 1 :])
        end = len(text) if match is None else start + 1 + match.start()
        return text[start:end]

    def heredoc(self, marker):
        text = self.text()
        at = text.index(marker)
        start = text.rfind("python3 - <<'PY'\n", 0, at)
        lines = text[start:].splitlines()[1:]
        body = []
        for line in lines:
            if line.strip() == "PY":
                break
            body.append(line)
        return textwrap.dedent("\n".join(body) + "\n")

    def test_script_is_portable_and_does_not_shell_out(self):
        source = SCRIPT.read_text(encoding="utf-8").lower()
        self.assertNotIn("pcoc", source)
        self.assertNotIn("groundinglab", source)
        for banned in ("subprocess", "os.system", "shell=true", "eval(", "exec(", "pickle"):
            self.assertNotIn(banned, source)

    def test_pr_body_job_is_isolated(self):
        block = self.job("pr-body")
        self.assertIn("contents: read", block)
        self.assertIn("pull-requests: read", block)
        self.assertIn("issues: read", block)
        self.assertIn("persist-credentials: false", block)
        self.assertIn("github.token", block)
        self.assertNotIn("secrets.", block)
        self.assertNotIn("G1_PATTERNS", block)
        self.assertNotIn("github.event.pull_request.body", block)
        runs = re.findall(r"(?m)^\s+run:\s*(.+)$", block)
        self.assertEqual(runs, ["python3 .github/scripts/pr_body_check.py"])
        self.assertNotIn("${{", "\n".join(runs))
        self.assertIn(
            "github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository",
            block,
        )
        g1 = self.job("g1")
        self.assertIn("secrets.G1_PATTERNS", g1)
        self.assertIn("bash .github/scripts/g1.sh", g1)

    def test_trigger_includes_edited_and_the_defaults(self):
        text = self.text()
        self.assertNotIn("github.event.pull_request.body", text)
        types = re.search(r"types:\s*\[([^\]]+)\]", text)
        self.assertIsNotNone(types)
        found = [part.strip() for part in types.group(1).split(",")]
        self.assertEqual(found, ["opened", "synchronize", "reopened", "edited"])

    def test_job_guard_includes_pr_body(self):
        body = self.text().split("\njobs:\n", 1)[1]
        jobs = set(re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", body, re.M)) - {"ci-ok"}
        self.assertIn("pr-body", jobs)
        needs = {name: {"result": "success"} for name in jobs}
        proc = self.run_python(self.heredoc("guard ok"), {"NEEDS": json.dumps(needs)})
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("guard ok", proc.stdout)

    def needs(self, **overrides):
        names = [
            "changes", "spec", "sdk", "services", "desktop", "desktop-windows",
            "docs", "gitleaks", "g1", "pr-body", "retired-term", "xverify",
        ]
        out = {name: {"result": "success"} for name in names}
        for name, result in overrides.items():
            out[name]["result"] = result
        return out

    def run_python(self, source, env_extra):
        env = os.environ.copy()
        env.update(env_extra)
        return subprocess.run(
            [sys.executable, "-c", source],
            capture_output=True,
            text=True,
            env=env,
            cwd=ROOT,
            check=False,
        )

    def aggregate(self, event, fork="false", **overrides):
        env = {
            "NEEDS": json.dumps(self.needs(**overrides)),
            "EVENT_NAME": event,
            "FORK_PR": fork,
        }
        return self.run_python(self.heredoc("ci-ok: all required jobs passed"), env)

    def test_aggregate_accepts_skip_off_pull_request(self):
        for event in ("push", "schedule", "workflow_dispatch"):
            with self.subTest(event=event):
                proc = self.aggregate(event, **{"pr-body": "skipped"})
                self.assertEqual(proc.returncode, 0, proc.stderr)
                self.assertIn("ci-ok: all required jobs passed or were skipped", proc.stdout)

    def test_aggregate_requires_pr_body_on_a_pull_request(self):
        failed = self.aggregate("pull_request", **{"pr-body": "failure"})
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual(failed.stderr.count("pr-body=failure"), 1)
        skipped = self.aggregate("pull_request", **{"pr-body": "skipped"})
        self.assertNotEqual(skipped.returncode, 0)
        self.assertIn("pr-body=skipped", skipped.stderr)
        ok = self.aggregate("pull_request")
        self.assertEqual(ok.returncode, 0, ok.stderr)

    def test_aggregate_push_failure_still_fails(self):
        proc = self.aggregate("push", **{"pr-body": "failure"})
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("pr-body=failure", proc.stderr)

    def test_aggregate_ignores_desktop_and_keeps_g1_fork_rule(self):
        desktop = self.aggregate("pull_request", desktop="failure")
        self.assertEqual(desktop.returncode, 0, desktop.stderr)
        fork_skip = self.aggregate("pull_request", "true", g1="skipped", **{"pr-body": "skipped"})
        self.assertEqual(fork_skip.returncode, 0, fork_skip.stderr)
        self.assertIn("g1: skipped on fork PR (accepted)", fork_skip.stdout)
        self.assertIn("pr-body: skipped on fork PR (accepted)", fork_skip.stdout)
        same_repo = self.aggregate("pull_request", "false", g1="skipped")
        self.assertNotEqual(same_repo.returncode, 0)
        self.assertIn("g1=skipped", same_repo.stderr)

    def test_unknown_event_fails_closed(self):
        proc = self.aggregate("deployment")
        self.assertNotEqual(proc.returncode, 0)


class ContribTests(unittest.TestCase):
    def test_one_line_names_the_rule_and_links_the_gate(self):
        text = (ROOT / "CONTRIBUTING.md").read_text(encoding="utf-8")
        hits = [
            line for line in text.splitlines()
            if "Risk tier:" in line and "Red Hat verdict:" in line and "White Hat verdict:" in line
        ]
        self.assertEqual(len(hits), 1)
        self.assertIn("SPEC / L5-shaped", hits[0])
        self.assertIn(
            "https://github.com/groundinglab/ops/blob/main/devops/INTENT_AND_RISK_GATE.md",
            hits[0],
        )


if __name__ == "__main__":
    unittest.main()
