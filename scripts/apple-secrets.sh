#!/usr/bin/env bash
# Carga en el repo los secrets para firmar y notarizar la app de macOS (macos.yml).
# Se corre una vez, en la máquina que tiene los certificados, con `gh` autenticado:
#
#   scripts/apple-secrets.sh ~/apple-certs
#
# El certificado Developer ID se toma, en este orden, de:
#   1. apple_devid.key (llave privada) + el .cer/.pem de Developer ID Application que le
#      corresponda (se comprueba que la llave pública coincida). No pide la contraseña del .p12.
#   2. devid.p12 y su contraseña (o si se fuerza con USE_P12=1).
# En los dos casos se arma un .p12 nuevo con una contraseña aleatoria: ésa es la que se sube.
#
# No imprime ni escribe a disco llaves ni contraseñas: lo que se teclea se pide con
# `read -s` y todo pasa a `gh secret set` por stdin.
set -euo pipefail

DIR="${1:-$HOME/apple-certs}"
REPO="${REPO:-ellaguno/iureapps}"
TEAM_ID="PLNS58WDQF"
ASC_KEY_ID="596256L8PU"
ASC_ISSUER_ID="12352a5c-64b1-4d69-8a4f-374ce2bb62f8"
KEY="$DIR/apple_devid.key"
P12="$DIR/devid.p12"
P8="$DIR/AuthKey_${ASC_KEY_ID}.p8"

die() { echo "$*" >&2; exit 1; }
[[ -r "$P8" ]] || die "No encuentro $P8"
command -v gh >/dev/null || die "Falta gh (GitHub CLI)"
command -v openssl >/dev/null || die "Falta openssl"
gh auth status >/dev/null

KEY_PEM="" CERT_PEM=""

if [[ -r "$KEY" && "${USE_P12:-}" != 1 ]]; then
  echo "Usando $KEY y su certificado."
  if ! KEY_PEM=$(openssl pkey -in "$KEY" -passin pass: 2>/dev/null); then
    IFS= read -rsp "Frase de la llave apple_devid.key: " KEY_PASS; echo
    export KEY_PASS
    KEY_PEM=$(openssl pkey -in "$KEY" -passin env:KEY_PASS 2>/dev/null) ||
      die "No se pudo abrir $KEY (¿frase incorrecta?)"
    unset KEY_PASS
  fi
  PUB=$(printf '%s\n' "$KEY_PEM" | openssl pkey -pubout)
  shopt -s nullglob
  for c in "$DIR"/*.cer "$DIR"/*.crt "$DIR"/*.pem; do
    pem=$(openssl x509 -in "$c" -inform DER 2>/dev/null ||
          openssl x509 -in "$c" -inform PEM 2>/dev/null) || continue
    printf '%s\n' "$pem" | openssl x509 -noout -subject | grep -q "Developer ID Application" || continue
    [[ "$(printf '%s\n' "$pem" | openssl x509 -noout -pubkey)" == "$PUB" ]] || continue
    CERT_PEM=$pem
    echo "Certificado: $(basename "$c")"
    break
  done
  [[ -n "$CERT_PEM" ]] ||
    die "No hay en $DIR un .cer/.pem de Developer ID Application que corresponda a apple_devid.key"
else
  [[ -r "$P12" ]] || die "No encuentro $KEY ni $P12"
  echo "Usando $P12."
  # IFS= para no recortar espacios al principio o al final de la contraseña.
  IFS= read -rsp "Contraseña de devid.p12: " OLD_PASSWORD; echo
  export OLD_PASSWORD
  LEGACY=()
  if ! ERR1=$(openssl pkcs12 -in "$P12" -passin env:OLD_PASSWORD -nodes 2>&1 >/dev/null); then
    if ERR2=$(openssl pkcs12 -in "$P12" -passin env:OLD_PASSWORD -nodes -legacy 2>&1 >/dev/null); then
      LEGACY=(-legacy)
    else
      # Los mensajes de openssl no incluyen la contraseña.
      echo "No se pudo abrir $P12 ($(openssl version))." >&2
      echo "--- openssl:" >&2; echo "$ERR1" | head -5 >&2
      echo "--- openssl -legacy:" >&2; echo "$ERR2" | head -5 >&2
      die "«Mac verify error» = contraseña incorrecta; «unsupported»/«legacy» = falta el proveedor legacy de OpenSSL 3."
    fi
  fi
  KEY_PEM=$(openssl pkcs12 -in "$P12" -passin env:OLD_PASSWORD -nodes -nocerts "${LEGACY[@]}" 2>/dev/null | openssl pkey)
  CERT_PEM=$(openssl pkcs12 -in "$P12" -passin env:OLD_PASSWORD -nokeys -clcerts "${LEGACY[@]}" 2>/dev/null | openssl x509)
  unset OLD_PASSWORD
fi

SUBJECT=$(printf '%s\n' "$CERT_PEM" | openssl x509 -noout -subject)
grep -q "Developer ID Application" <<<"$SUBJECT" || die "No es un certificado Developer ID Application"
grep -q "$TEAM_ID" <<<"$SUBJECT" || die "El certificado no es del equipo $TEAM_ID"
printf '%s\n' "$CERT_PEM" | openssl x509 -noout -checkend 0 >/dev/null || die "El certificado ya venció"
echo "$SUBJECT"
printf '%s\n' "$CERT_PEM" | openssl x509 -noout -enddate

# .p12 nuevo, con contraseña aleatoria y cifrado 3DES/SHA1: `security import` de macOS no
# siempre lee los que OpenSSL 3 genera por defecto (AES-256 + PBKDF2). Sólo en memoria.
P12_PASSWORD=$(openssl rand -base64 24 | tr -d '\n')
export P12_PASSWORD
P12_B64="$(openssl pkcs12 -export -passout env:P12_PASSWORD -name "Developer ID Application" \
  -in <(printf '%s\n' "$CERT_PEM") -inkey <(printf '%s\n' "$KEY_PEM") \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 | base64 -w0)"
unset KEY_PEM CERT_PEM
# Comprobación: el .p12 nuevo abre con su contraseña y trae la llave.
printf '%s' "$P12_B64" | base64 -d |
  openssl pkcs12 -passin env:P12_PASSWORD -nodes 2>/dev/null | grep -q "PRIVATE KEY" ||
  die "No se pudo armar el .p12"

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
