# Forkctl Rebase Review

- Target: `{{ target }}`
- Old base: `{{ old_base }}`
- Old tip: `{{ old_tip }}`
- New base: `{{ new_base }}`
- New tip: `{{ new_tip }}`
- Recovery tag: `{{ recovery_tag }}`
- Structural verification: passed
- Semantic verification: pending consumer checks

## Exports

{% for export in exports %}- `{{ export.path }}` — `{{ export.hash }}`
{% else %}- None
{% endfor %}
## Range diff

```diff
{{ range_diff }}
```

