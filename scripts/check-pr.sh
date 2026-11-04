#!/usr/bin/env bash
# Enforces git flow branch names and Conventional Commits on a pull request.
set -euo pipefail

: "${BASE_REF:?}" "${HEAD_REF:?}" "${BASE_SHA:?}" "${HEAD_SHA:?}"

case "$BASE_REF" in
  main) allowed='^(release|hotfix)/.+$' ;;
  develop) allowed='^(feature|bugfix|release|hotfix)/.+$|^main$' ;;
  *)
    echo "Pull requests must target main or develop, not '$BASE_REF'." >&2
    exit 1
    ;;
esac

if ! [[ "$HEAD_REF" =~ $allowed ]]; then
  echo "A pull request into '$BASE_REF' cannot come from '$HEAD_REF'." >&2
  exit 1
fi

pattern='^(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)(\([a-z0-9-]+\))?!?: .+$'
status=0
while IFS= read -r subject; do
  if ! [[ "$subject" =~ $pattern ]]; then
    echo "Not a Conventional Commit: $subject" >&2
    status=1
  fi
done < <(git log --no-merges --format=%s "$BASE_SHA..$HEAD_SHA")
exit "$status"
