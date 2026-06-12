#!/usr/bin/env bash
set -euo pipefail

if command -v gitleaks >/dev/null 2>&1; then
  exec gitleaks detect --source . --no-banner
fi

echo "gitleaks not found; running limited fallback scan" >&2
set +e
rg -n --hidden \
  --glob '!.git/' \
  --glob '!target/' \
  --glob '!dist/' \
  --glob '!build/' \
  --glob '!.venv/' \
  --glob '!downloads/' \
  --glob '!models/' \
  --glob '!docs/redesign/' \
  '(api[_-]?key\s*[:=]|access[_-]?token\s*[:=]|secret\s*[:=]|private[_-]?key\s*[:=]|BEGIN (RSA|OPENSSH|EC|DSA) PRIVATE KEY)' .
status=$?
set -e

if [[ "$status" -eq 0 ]]; then
  echo "potential secret-like strings found by fallback scan" >&2
  exit 1
fi
if [[ "$status" -ne 1 ]]; then
  exit "$status"
fi
