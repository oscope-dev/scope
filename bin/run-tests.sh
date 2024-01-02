#!/usr/bin/env bash

TEMP_DIR="$(mktemp -d)"
mkdir -p $TEMP_DIR/.pity

cat <<'EOF' > "${TEMP_DIR}/.pity/check.sh"
#!/bin/bash

if [ -f "exists.txt" ]; then
  exit 0
else
  exit 1
fi
EOF

cat <<'EOF' > "${TEMP_DIR}/.pity/fix.sh"
#!/bin/bash

touch exists.txt
EOF

chmod +x "${TEMP_DIR}/.pity/fix.sh"
chmod +x "${TEMP_DIR}/.pity/check.sh"

cat <<EOF > "${TEMP_DIR}/.pity/check.yaml"
apiVersion: pity.github.com/v1alpha
kind: PityDoctorCheck
metadata:
  name: test
spec:
  check:
    target: check.sh
  fix:
    target: fix.sh
  description: Check if file exists
  help: Run 'fix.sh'
EOF

cargo run --bin pity -- --working-dir "${TEMP_DIR}" doctor run -dd --fix
EXIT_CODE=$?

if [[ "${EXIT_CODE}" != "1" ]]; then
  >&2 echo "Test failed"
  exit 1
fi

cargo run --bin pity -- --working-dir "${TEMP_DIR}" doctor run -dd --fix
EXIT_CODE=$?

exit ${EXIT_CODE}