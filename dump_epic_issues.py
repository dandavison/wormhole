#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.12"
# dependencies = ["httpx"]
# ///
"""Dump all issues in specified JIRA epics to JSON."""

import base64
import json
import os
import sys

import httpx


def get_auth_header() -> str:
    email = os.environ.get("JIRA_EMAIL")
    token = os.environ.get("JIRA_TOKEN")
    if not email or not token:
        raise ValueError("JIRA_EMAIL and JIRA_TOKEN must be set")
    credentials = f"{email}:{token}"
    encoded = base64.b64encode(credentials.encode()).decode()
    return f"Basic {encoded}"


def get_issues_in_epic(epic_key: str, instance: str = "temporalio") -> list[dict]:
    """Fetch all issues that belong to the specified epic."""
    url = f"https://{instance}.atlassian.net/rest/api/3/search/jql"
    headers = {
        "Authorization": get_auth_header(),
        "Content-Type": "application/json",
    }
    # Try both "Epic Link" (classic) and parent (next-gen) approaches
    jql = f'"Epic Link" = {epic_key} OR parent = {epic_key} ORDER BY key'

    issue_keys = []
    start_at = 0
    max_results = 100

    with httpx.Client(timeout=30) as client:
        # First, get all issue keys
        while True:
            params = {
                "jql": jql,
                "startAt": start_at,
                "maxResults": max_results,
                "fields": "key",
            }
            response = client.get(url, headers=headers, params=params)
            response.raise_for_status()
            data = response.json()

            issues = data.get("issues", [])
            issue_keys.extend([i["key"] for i in issues])

            total = data.get("total", 0)
            start_at += len(issues)
            if start_at >= total or not issues:
                break

        # Then fetch full details for each issue
        all_issues = []
        for key in issue_keys:
            print(f"    Fetching {key}...", file=sys.stderr)
            issue = get_issue_details(key, instance, headers)
            all_issues.append(issue)

    return all_issues


CORE_FIELDS = (
    "key,summary,status,assignee,issuetype,priority,created,updated,description"
)


def get_issue_details(key: str, instance: str, headers: dict) -> dict:
    """Fetch core details for a single issue."""
    url = f"https://{instance}.atlassian.net/rest/api/3/issue/{key}"
    params = {"fields": CORE_FIELDS}

    with httpx.Client(timeout=30) as client:
        response = client.get(url, headers=headers, params=params)
        response.raise_for_status()
        return response.json()


def get_epic_details(epic_key: str, instance: str = "temporalio") -> dict:
    """Fetch the epic issue itself."""
    url = f"https://{instance}.atlassian.net/rest/api/3/issue/{epic_key}"
    headers = {
        "Authorization": get_auth_header(),
        "Content-Type": "application/json",
    }
    params = {"fields": CORE_FIELDS}

    with httpx.Client(timeout=30) as client:
        response = client.get(url, headers=headers, params=params)
        response.raise_for_status()
        return response.json()


def main():
    epic_keys = ["ACT-1", "ACT-626"]

    result = {}
    for epic_key in epic_keys:
        print(f"Fetching epic {epic_key}...", file=sys.stderr)
        epic = get_epic_details(epic_key)
        print(f"Fetching issues in {epic_key}...", file=sys.stderr)
        issues = get_issues_in_epic(epic_key)
        print(f"  Found {len(issues)} issues", file=sys.stderr)
        result[epic_key] = {
            "epic": epic,
            "issues": issues,
        }

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
