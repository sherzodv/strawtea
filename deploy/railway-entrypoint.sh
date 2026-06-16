#!/bin/sh
set -eu

js_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

cat >"${STATIC_DIR}/config.js" <<EOF
window.__STRAWTEA_CONFIG__ = {
  VITE_API_BASE_URL: "$(js_escape "${VITE_API_BASE_URL:-}")",
  VITE_SUPABASE_URL: "$(js_escape "${VITE_SUPABASE_URL:-}")",
  VITE_SUPABASE_ANON_KEY: "$(js_escape "${VITE_SUPABASE_ANON_KEY:-}")"
};
EOF

exec /app/strawtea-be
