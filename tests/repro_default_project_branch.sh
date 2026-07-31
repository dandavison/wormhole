#!/usr/bin/env bash
# Run: tests/repro_default_project_branch.sh

set -euo pipefail

report_dir=$(mktemp -d)
trap 'rm -rf "$report_dir"' EXIT
stdout_file="$report_dir/stdout"
stderr_file="$report_dir/stderr"
command='WORMHOLE_TEST=1 WORMHOLE_EDITOR=none cargo test --test test_integration test_default_project_branch_resolution -- --exact --nocapture'

if WORMHOLE_TEST=1 WORMHOLE_EDITOR=none cargo test \
    --test test_integration \
    test_default_project_branch_resolution \
    -- \
    --exact \
    --nocapture >"$stdout_file" 2>"$stderr_file"; then
    status=0
else
    status=$?
fi

printf '# Branch-only task resolution\n\n'
printf 'This checks that `:branch` opens an existing task in any project and otherwise creates the task in `WORMHOLE_DEFAULT_PROJECT`.\n\n'
printf '## Scenario\n\n'
printf 'Expected: the command exits successfully after both branch-only forms resolve as described.\n\n'
printf '```bash\n%s\n```\n\n' "$command"
printf '### stdout\n\n```\n'
sed -n '1,240p' "$stdout_file"
printf '```\n\n### stderr\n\n```\n'
sed -n '1,240p' "$stderr_file"
printf '```\n\nExit code: `%s`\n\n' "$status"
printf '## Finding\n\n'
if ((status == 0)); then
    printf 'Branch-only task resolution behaves as expected.\n'
else
    printf 'Branch-only task resolution is not implemented: the targeted integration test failed.\n'
fi

exit "$status"
