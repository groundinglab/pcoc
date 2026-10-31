#!/usr/bin/env python3
"""Check a pull request body for a Risk tier line and verdict comment links.

High and SPEC / L5-shaped bodies need one Red Hat verdict line and one
White Hat verdict line. Each line carries one URL of the form
https://<host>/<org>/<repo>/(pull|issues)/<n>#issuecomment-<id>, where
<n> is this pull request or an issue in closingIssuesReferences.
The comment must exist and its issue_url must name that same issue.

Org, repo, and host come from the event or from GITHUB_REPOSITORY.
The API base is GITHUB_API_URL (default https://api.github.com).
No repository name is hard-coded. The body is read as JSON data.
Only the fixed messages below are printed.

Usage:
  pr_body_check.py [--event PATH] [--comments-file PATH] [--linked-issues LIST]

--comments-file and --linked-issues are local fixtures. The workflow
does not pass them. A comments file maps a comment id to
{"issue_url": "..."} or to null (404). With --comments-file and no
--linked-issues, the linked-issue set is empty and no network is used.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

MISSING_TIER = "missing Risk tier: line"
BAD_TIER = "Risk tier: value is not Low, Medium, High, or SPEC / L5-shaped"
CLOSING_REFS_UNREAD = "closingIssuesReferences: could not be read"
NO_TOKEN = "GITHUB_TOKEN is not set"
OK = "pr-body check: ok"
NOT_A_PULL_REQUEST = "pr-body check: not a pull request"
EVENT_UNREADABLE = "event JSON could not be read"
REPOSITORY_MISSING = "repository is missing"
FIXTURE_UNREADABLE = "comments fixture could not be read"

_LABELS = ("Red Hat", "White Hat")
_TIER_VALUES = {
    "low": "low",
    "medium": "medium",
    "high": "high",
    "spec / l5-shaped": "spec / l5-shaped",
}
_NEEDS_VERDICTS = frozenset({"high", "spec / l5-shaped"})
_TIER_LINE = re.compile(r"^risk tier:\s*(.*?)\s*$", re.IGNORECASE)
_VERDICT_LINE = re.compile(
    r"^(red hat verdict|white hat verdict):\s*(.*?)\s*$", re.IGNORECASE
)
_LABEL_FOR = {
    "red hat verdict": "Red Hat",
    "white hat verdict": "White Hat",
}
_TARGET = re.compile(
    r"^https://(?P<host>[A-Za-z0-9.-]+)/(?P<org>[A-Za-z0-9_.-]+)/(?P<repo>[A-Za-z0-9_.-]+)"
    r"/(?P<kind>pull|issues)/(?P<num>[1-9][0-9]*)(?:#(?P<frag>[A-Za-z0-9._-]*))?$",
    re.IGNORECASE,
)
_MARKDOWN_LINK = re.compile(r"^\[([^\]\r\n]*)\]\(([^)\s]+)\)$")
_CLOSING_QUERY = """
query ($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      closingIssuesReferences(first: 100) {
        pageInfo { hasNextPage }
        nodes { number }
      }
    }
  }
}
""".strip()


def missing_verdict(label: str) -> str:
    return _fmt("missing {label} verdict: line", label)


def malformed_verdict(label: str) -> str:
    return _fmt("{label} verdict: malformed", label)


def comment_not_found(label: str) -> str:
    return _fmt("{label} verdict: comment not found", label)


def comment_wrong_issue(label: str) -> str:
    return _fmt("{label} verdict: comment is on the wrong issue", label)


def comment_unread(label: str) -> str:
    return _fmt("{label} verdict: comment could not be read", label)


def _fmt(template: str, label: str) -> str:
    if label not in _LABELS:
        raise ValueError("unexpected verdict label")
    return template.format(label=label)


class ApiError(Exception):
    pass


class NoToken(ApiError):
    pass


class CommentMissing(Exception):
    pass


class ParsedVerdict:
    def __init__(self, label, status, detail, number=None, comment_id=None):
        self.label = label
        self.status = status
        self.detail = detail
        self.number = number
        self.comment_id = comment_id


class CommentRecord:
    def __init__(self, org, repo, number):
        self.org = org
        self.repo = repo
        self.number = number


def normalize_line(raw):
    return raw.replace("**", "").replace("__", "").strip()


def parse_tier(body):
    """Return (tier, error). tier is low, medium, high, or spec / l5-shaped."""
    if not isinstance(body, str):
        body = ""
    found = []
    bad = False
    for raw in body.splitlines():
        match = _TIER_LINE.match(normalize_line(raw))
        if not match:
            continue
        value = re.sub(r"\s+", " ", match.group(1).strip()).lower()
        canonical = _TIER_VALUES.get(value)
        if canonical is None:
            bad = True
            continue
        found.append(canonical)
    if not found:
        return None, (BAD_TIER if bad else MISSING_TIER)
    unique = set(found)
    if len(unique) != 1:
        return None, BAD_TIER
    return found[0], None


def sole_url(value):
    """Return (url, detail). detail is ok, extra-url, or malformed."""
    value = value.strip()
    wrapped = _MARKDOWN_LINK.fullmatch(value)
    if wrapped:
        text, target = wrapped.group(1), wrapped.group(2)
        if re.search(r"https?://", text, re.IGNORECASE):
            return None, "extra-url"
        value = target.strip()
    urls = re.findall(r"https?://\S+", value, re.IGNORECASE)
    if len(urls) != 1:
        return None, ("extra-url" if urls else "malformed")
    if value != urls[0]:
        return None, "extra-url"
    if not value.lower().startswith("https://"):
        return None, "malformed"
    return value, "ok"


def classify_url(url, org, repo, host, allowed):
    """Return (detail, number, comment_id).

    allowed is a set of issue numbers, or None to skip the number check.
    """
    match = _TARGET.fullmatch(url)
    if not match:
        return "malformed", None, None
    if match.group("host").lower() != host.lower():
        return "wrong-host", None, None
    if match.group("org").lower() != org.lower():
        return "wrong-org", None, None
    if match.group("repo").lower() != repo.lower():
        return "wrong-repo", None, None
    number = int(match.group("num"))
    frag = match.group("frag") or ""
    if frag == "":
        return "missing-anchor", number, None
    folded = frag.lower()
    if folded.startswith("pullrequestreview-"):
        return "review-link", number, None
    comment = re.fullmatch(r"issuecomment-([1-9][0-9]*)", folded)
    if not comment:
        return "malformed", number, None
    comment_id = comment.group(1)
    if allowed is not None and number not in allowed:
        return "wrong-number", number, comment_id
    return "ok", number, comment_id


def parse_verdicts(body, org, repo, host, allowed):
    """Return Red Hat then White Hat. status is ok, missing, or malformed."""
    if not isinstance(body, str):
        body = ""
    found = {label: [] for label in _LABELS}
    for raw in body.splitlines():
        match = _VERDICT_LINE.match(normalize_line(raw))
        if not match:
            continue
        label = _LABEL_FOR[match.group(1).lower()]
        found[label].append(match.group(2))
    parsed = []
    for label in _LABELS:
        values = found[label]
        if not values:
            parsed.append(ParsedVerdict(label, "missing", "missing"))
            continue
        if len(values) != 1:
            parsed.append(ParsedVerdict(label, "malformed", "duplicate"))
            continue
        url, url_detail = sole_url(values[0])
        if url is None:
            parsed.append(ParsedVerdict(label, "malformed", url_detail))
            continue
        detail, number, comment_id = classify_url(url, org, repo, host, allowed)
        status = "ok" if detail == "ok" else "malformed"
        parsed.append(ParsedVerdict(label, status, detail, number, comment_id))
    return parsed


def _same_issue(record, org, repo, number):
    return (
        record.org.lower() == org.lower()
        and record.repo.lower() == repo.lower()
        and record.number == number
    )


def _parser_errors(verdicts, suppress_numbers):
    errors = []
    pending = []
    for verdict in verdicts:
        if verdict.status == "missing":
            errors.append(missing_verdict(verdict.label))
        elif verdict.status == "malformed":
            if (
                suppress_numbers is not None
                and verdict.detail == "wrong-number"
                and verdict.number in suppress_numbers
            ):
                continue
            errors.append(malformed_verdict(verdict.label))
        else:
            pending.append(verdict)
    return errors, pending


def _comment_error(verdict, org, repo, transport):
    try:
        record = transport.comment(org, repo, verdict.comment_id)
    except NoToken:
        raise
    except CommentMissing:
        return comment_not_found(verdict.label)
    except ApiError:
        return comment_unread(verdict.label)
    if not _same_issue(record, org, repo, verdict.number):
        return comment_wrong_issue(verdict.label)
    return None


def run_check(body, org, repo, number, host, transport, linked_override):
    """Return fixed error messages. Empty means the body passes.

    linked_override is a set when the caller already knows
    closingIssuesReferences (including an empty set). None asks the
    transport when a verdict URL names some other issue.
    """
    _tier, tier_error = parse_tier(body)
    if tier_error:
        return [tier_error]
    if _tier not in _NEEDS_VERDICTS:
        return []
    host = host or "github.com"
    probed = parse_verdicts(body, org, repo, host, None)
    foreign = {
        verdict.number
        for verdict in probed
        if verdict.detail == "ok" and verdict.number != number
    }
    linked = set()
    suppress = None
    prefix = []
    if linked_override is not None:
        linked = set(linked_override)
    elif foreign:
        try:
            linked = set(transport.closing_issues(org, repo, number))
        except NoToken:
            verdicts = parse_verdicts(body, org, repo, host, {number})
            errors, _pending = _parser_errors(verdicts, foreign)
            errors.append(NO_TOKEN)
            return errors
        except ApiError:
            prefix.append(CLOSING_REFS_UNREAD)
            suppress = foreign
            linked = set()
    verdicts = parse_verdicts(body, org, repo, host, {number, *linked})
    errors, pending = _parser_errors(verdicts, suppress)
    for verdict in pending:
        try:
            message = _comment_error(verdict, org, repo, transport)
        except NoToken:
            errors.append(NO_TOKEN)
            break
        if message:
            errors.append(message)
    return prefix + errors


def parse_issue_url(issue_url, api):
    prefix = api.rstrip("/") + "/repos/"
    if not isinstance(issue_url, str) or not issue_url.startswith(prefix):
        return None
    match = re.fullmatch(
        r"([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)/issues/([1-9][0-9]*)",
        issue_url[len(prefix) :],
    )
    if not match:
        return None
    return CommentRecord(match.group(1), match.group(2), int(match.group(3)))


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise urllib.error.HTTPError(req.full_url, code, msg, headers, fp)


class GitHubTransport:
    def __init__(self, api, token):
        self.api = (api or "https://api.github.com").rstrip("/")
        self.token = token or ""

    def _call(self, url, payload):
        if not self.token:
            raise NoToken()
        headers = {
            "Accept": "application/vnd.github+json",
            "Authorization": "Bearer " + self.token,
            "User-Agent": "pr-body-check",
            "X-GitHub-Api-Version": "2022-11-28",
        }
        data = None
        method = "GET"
        if payload is not None:
            data = json.dumps(payload).encode("utf-8")
            headers["Content-Type"] = "application/json"
            method = "POST"
        request = urllib.request.Request(url, data=data, headers=headers, method=method)
        opener = urllib.request.build_opener(_NoRedirect)
        try:
            with opener.open(request, timeout=15) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as exc:
            return exc.code, exc.read()
        except urllib.error.URLError as exc:
            raise ApiError("request failed") from exc

    def comment(self, org, repo, comment_id):
        if not re.fullmatch(r"[1-9][0-9]*", str(comment_id)):
            raise ApiError("comment id")
        url = (
            self.api
            + "/repos/"
            + urllib.parse.quote(org, safe="")
            + "/"
            + urllib.parse.quote(repo, safe="")
            + "/issues/comments/"
            + urllib.parse.quote(str(comment_id), safe="")
        )
        status, raw = self._call(url, None)
        if status == 404:
            raise CommentMissing()
        if status != 200:
            raise ApiError("comment status")
        try:
            payload = json.loads(raw.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError) as exc:
            raise ApiError("comment json") from exc
        if not isinstance(payload, dict):
            raise ApiError("comment json")
        got = payload.get("id")
        if got is not None and str(got) != str(comment_id):
            raise ApiError("comment id mismatch")
        record = parse_issue_url(payload.get("issue_url"), self.api)
        if record is None:
            raise ApiError("issue_url")
        return record

    def closing_issues(self, org, repo, number):
        status, raw = self._call(
            self.api + "/graphql",
            {
                "query": _CLOSING_QUERY,
                "variables": {"owner": org, "name": repo, "number": number},
            },
        )
        if status != 200:
            raise ApiError("graphql status")
        try:
            payload = json.loads(raw.decode("utf-8"))
        except (UnicodeError, json.JSONDecodeError) as exc:
            raise ApiError("graphql json") from exc
        if not isinstance(payload, dict) or payload.get("errors"):
            raise ApiError("graphql errors")
        repository = (payload.get("data") or {}).get("repository")
        pull = None if not isinstance(repository, dict) else repository.get("pullRequest")
        if not isinstance(pull, dict):
            raise ApiError("graphql pull")
        refs = pull.get("closingIssuesReferences")
        if not isinstance(refs, dict):
            raise ApiError("graphql refs")
        page = refs.get("pageInfo")
        if not isinstance(page, dict) or page.get("hasNextPage") is not False:
            raise ApiError("graphql page")
        nodes = refs.get("nodes")
        if not isinstance(nodes, list):
            raise ApiError("graphql nodes")
        numbers = set()
        for node in nodes:
            if not isinstance(node, dict) or not isinstance(node.get("number"), int):
                raise ApiError("graphql node")
            if isinstance(node["number"], bool):
                raise ApiError("graphql node")
            numbers.add(node["number"])
        return numbers


class FixtureTransport:
    """Offline comment map. closing_issues is never used in fixture mode."""

    def __init__(self, comments, api):
        self.comments = comments
        self.api = api

    def comment(self, org, repo, comment_id):
        if comment_id not in self.comments:
            raise CommentMissing()
        row = self.comments[comment_id]
        if row is None:
            raise CommentMissing()
        if not isinstance(row, dict):
            raise ApiError("fixture row")
        record = parse_issue_url(row.get("issue_url"), self.api)
        if record is None:
            raise ApiError("fixture issue_url")
        return record

    def closing_issues(self, org, repo, number):
        raise ApiError("fixture has no closing refs")


def load_comments(path, api):
    try:
        with open(path, encoding="utf-8") as handle:
            payload = json.load(handle)
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ApiError("fixture") from exc
    if not isinstance(payload, dict):
        raise ApiError("fixture")
    comments = {}
    for key, row in payload.items():
        if not re.fullmatch(r"[1-9][0-9]*", str(key)):
            raise ApiError("fixture")
        if row is not None and not isinstance(row, dict):
            raise ApiError("fixture")
        comments[str(key)] = row
    return FixtureTransport(comments, api)


def parse_linked_issues(text):
    if text is None:
        return None
    if text.strip() == "":
        return set()
    numbers = set()
    for part in text.split(","):
        part = part.strip()
        if not re.fullmatch(r"[1-9][0-9]*", part):
            raise ValueError("linked issue")
        numbers.add(int(part))
    return numbers


def context_from_event(event):
    """Return (org, repo, number, body, host) or None when event is not a PR."""
    if not isinstance(event, dict) or not isinstance(event.get("pull_request"), dict):
        return None
    pull = event["pull_request"]
    number = pull.get("number")
    if isinstance(number, bool) or not isinstance(number, int) or number < 1:
        raise ValueError("number")
    repository = event.get("repository") if isinstance(event.get("repository"), dict) else {}
    owner = repository.get("owner") if isinstance(repository.get("owner"), dict) else {}
    org = owner.get("login") if isinstance(owner.get("login"), str) else ""
    repo = repository.get("name") if isinstance(repository.get("name"), str) else ""
    if not org or not repo:
        full = os.environ.get("GITHUB_REPOSITORY", "")
        if full.count("/") == 1:
            org, repo = full.split("/", 1)
    if not org or not repo:
        raise ValueError("repository")
    body = pull.get("body")
    if body is None:
        body = ""
    if not isinstance(body, str):
        body = ""
    host = "github.com"
    html = repository.get("html_url")
    if isinstance(html, str) and html:
        hostname = urllib.parse.urlparse(html).hostname
        if hostname:
            host = hostname
    return org, repo, number, body, host


def _emit(messages):
    for message in messages:
        if "\n" in message or "\r" in message:
            print("::error::pr-body check: failed")
            continue
        print("::error::" + message)


def main(argv=None):
    parser = argparse.ArgumentParser(description="Check a pull request body for Risk tier and verdict links.")
    parser.add_argument("--event", default=os.environ.get("GITHUB_EVENT_PATH"))
    parser.add_argument("--comments-file", default=None)
    parser.add_argument("--linked-issues", default=None)
    args = parser.parse_args(argv)
    if not args.event:
        _emit([EVENT_UNREADABLE])
        return 1
    try:
        with open(args.event, encoding="utf-8") as handle:
            event = json.load(handle)
    except (OSError, UnicodeError, json.JSONDecodeError):
        _emit([EVENT_UNREADABLE])
        return 1
    try:
        ctx = context_from_event(event)
    except ValueError as exc:
        code = str(exc)
        _emit([REPOSITORY_MISSING if code == "repository" else EVENT_UNREADABLE])
        return 1
    if ctx is None:
        print(NOT_A_PULL_REQUEST)
        return 0
    org, repo, number, body, host = ctx
    api = os.environ.get("GITHUB_API_URL", "https://api.github.com")
    try:
        if args.comments_file:
            transport = load_comments(args.comments_file, api)
            linked = parse_linked_issues(args.linked_issues)
            if linked is None:
                linked = set()
        else:
            transport = GitHubTransport(api, os.environ.get("GITHUB_TOKEN", ""))
            linked = parse_linked_issues(args.linked_issues)
    except (ApiError, ValueError):
        _emit([FIXTURE_UNREADABLE])
        return 1
    errors = run_check(body, org, repo, number, host, transport, linked)
    if errors:
        _emit(errors)
        return 1
    print(OK)
    return 0


if __name__ == "__main__":
    sys.exit(main())
