Forkctl proposal for `{{ branch }}`. Do not merge this pull request: promote the exact candidate with `mise run fork publish --promote` from a clone whose `HEAD` is the candidate.

- Candidate: `{{ candidate }}`
- Replaces downstream tip: `{{ downstream_tip }}`
- Upstream base: `{{ base_selector }}` at `{{ base_commit }}`

## Patches

{% for patch in patches -%}
- `{{ patch.name }}` ({{ patch.kind }}): {{ patch.purpose }}
{% endfor -%}
