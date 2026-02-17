There are review comments on PR {{repo}}#{{pr}}.

Use gh to read the PR comments and diff.

List comments: `gh api repos/{{repo}}/pulls/{{pr}}/comments`

Reply to a comment: `gh api repos/{{repo}}/pulls/{{pr}}/comments/{comment_id}/replies -f body="your reply"`

Skip comments that start with the 🤖 emoji (those are from AI agents).
Reply to comments that have not already been adequately answered.
If appropriate, make commits addressing the feedback.
Prefix each of your PR comments with the 🤖 emoji.
