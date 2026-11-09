#!/usr/bin/env bash
# Carga en el repo los secrets para firmar y notarizar la app de macOS (macos.yml).
# Se corre una vez, en la máquina que tiene los certificados, con `gh` autenticado:
#
#   scripts/apple-secrets.sh ~/apple-certs
#
# No imprime ni escribe a disco llaves ni contraseñas: la contraseña del .p12 se pide con
# `read -s` y todo pasa a `gh secret set` por stdin.
set -euo pipefail

DIR="${1:-$HOME/apple-certs}"
REPO="${REPO:-ellaguno/iureapps}"
TEAM_ID="PLNS58WDQF"
ASC_KEY_ID="596256L8PU"
ASC_ISSUER_ID="12352a5c-64b1-4d69-8a4f-374ce2bb62f8"
P12="$DIR/devid.p12"
P8="$DIR/AuthKey_${ASC_KEY_ID}.p8"

for f in "$P12" "$P8"; do
  [[ -r "$f" ]] || { echo "No encuentro $f" >&2; exit 1; }
done
command -v gh >/dev/null || { echo "Falta gh (GitHub CLI)" >&2; exit 1; }
command -v openssl >/dev/null || { echo "Falta openssl" >&2; exit 1; }
gh auth status >/dev/null

# IFS= para no recortar espacios al principio o al final de la contraseña.
IFS= read -rsp "Contraseña de devid.p12: " P12_PASSWORD; echo
export P12_PASSWORD

# Lee el .p12 (con -legacy si es de OpenSSL antiguo) y comprueba contraseña y llave privada.
read_p12() { openssl pkcs12 -in "$P12" -passin env:P12_PASSWORD -nodes "$@" 2>/dev/null; }
LEGACY=()
if ! ERR1=$(openssl pkcs12 -in "$P12" -passin env:P12_PASSWORD -nodes 2>&1 >/dev/null); then
  if ERR2=$(openssl pkcs12 -in "$P12" -passin env:P12_PASSWORD -nodes -legacy 2>&1 >/dev/null); then
    LEGACY=(-legacy)
  else
    # Los mensajes de openssl no incluyen la contraseña.
    echo "No se pudo abrir $P12 ($(openssl version))." >&2
    echo "--- openssl:" >&2; echo "$ERR1" | head -5 >&2
    echo "--- openssl -legacy:" >&2; echo "$ERR2" | head -5 >&2
    echo "«Mac verify error» = contraseña incorrecta; «unsupported»/«legacy» = falta el" >&2
    echo "proveedor legacy de OpenSSL 3 (p. ej. paquete openssl-libs o libssl3 completo)." >&2
    exit 1
  fi
fi
read_p12 "${LEGACY[@]}" | grep -q "PRIVATE KEY" || { echo "El .p12 no trae la llave privada" >&2; exit 1; }
read_p12 "${LEGACY[@]}" -nokeys | openssl x509 -noout -subject | grep -q "Developer ID Application" ||
  { echo "El .p12 no es un certificado Developer ID Application" >&2; exit 1; }

# Se vuelve a cifrar con 3DES/SHA1: `security import` de macOS no siempre lee los .p12 que
# OpenSSL 3 genera por defecto (AES-256 + PBKDF2). Sólo en memoria y tuberías, nada a disco.
PEM="$(read_p12 "${LEGACY[@]}")"
P12_B64="$(openssl pkcs12 -export -passout env:P12_PASSWORD \
  -in <(printf '%s\n' "$PEM") -inkey <(printf '%s\n' "$PEM") \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 | base64 -w0)"
unset PEM
# Comprobación: el .p12 nuevo abre con la misma contraseña y trae la llave.
printf '%s' "$P12_B64" | base64 -d |
  openssl pkcs12 -passin env:P12_PASSWORD -nodes 2>/dev/null | grep -q "PRIVATE KEY" ||
  { echo "No se pudo volver a cifrar el .p12" >&2; exit 1; }

printf '%s' "$P12_B64" | gh secret set DEVELOPER_ID_P12_BASE64 --repo "$REPO"
printf '%s' "$P12_PASSWORD" | gh secret set DEVELOPER_ID_PASSWORD --repo "$REPO"
unset P12_B64 P12_PASSWORD

gh secret set ASC_PRIVATE_KEY --repo "$REPO" < "$P8"
openssl rand -base64 32 | tr -d '\n' | gh secret set KEYCHAIN_PASSWORD --repo "$REPO"
gh secret set APPLE_TEAM_ID --repo "$REPO" --body "$TEAM_ID"
gh secret set ASC_KEY_ID --repo "$REPO" --body "$ASC_KEY_ID"
gh secret set ASC_ISSUER_ID --repo "$REPO" --body "$ASC_ISSUER_ID"

echo "Listo. Secrets de $REPO:"
gh secret list --repo "$REPO"
