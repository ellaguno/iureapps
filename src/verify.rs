//! Verificación de las firmas minisign (`.sig`) que publica el actualizador de Tauri.

use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};

fn decode_b64_text(b64: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64 inválido: {e}"))?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

/// Verifica `file` contra la firma `sig_b64` (contenido del `.sig`) y la clave `pubkey_b64`.
/// Tauri firma en modo «prehashed», así que se lee el archivo por bloques.
pub fn verify_file(pubkey_b64: &str, sig_b64: &str, file: &std::path::Path) -> Result<(), String> {
    use std::io::Read;
    let pk = PublicKey::decode(&decode_b64_text(pubkey_b64)?)
        .map_err(|e| format!("clave pública: {e}"))?;
    let sig = Signature::decode(&decode_b64_text(sig_b64)?).map_err(|e| format!("firma: {e}"))?;
    let mut verifier = pk.verify_stream(&sig).map_err(|e| format!("firma: {e}"))?;
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        verifier.update(&buf[..n]);
    }
    verifier
        .finalize()
        .map_err(|_| "la firma no coincide: el archivo está dañado o alterado".into())
}
