//! Verificación de las firmas minisign (`.sig`) que publica el actualizador de Tauri y, para las
//! apps que no firman, de la suma SHA-256 que publican en `SHA256SUMS`.

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

/// Comprueba `file` contra su línea en `sums` (formato de `sha256sum`: «<hex>  <nombre>»).
pub fn verify_sha256(sums: &str, name: &str, file: &std::path::Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let expected = sums
        .lines()
        .filter_map(|l| l.split_once(char::is_whitespace))
        .find(|(_, n)| n.trim().trim_start_matches('*') == name)
        .map(|(h, _)| h.to_ascii_lowercase())
        .ok_or_else(|| format!("SHA256SUMS no incluye {name}"))?;
    let mut hasher = Sha256::new();
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got == expected {
        Ok(())
    } else {
        Err("la suma SHA-256 no coincide: el archivo está dañado o alterado".into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn checks_sha256sums() {
        let dir = std::env::temp_dir().join(format!("iureapps-sha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.deb");
        std::fs::write(&file, b"abc").unwrap();
        let sums = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  app.deb\n\
                    0000000000000000000000000000000000000000000000000000000000000000 *otra.exe\n";
        assert!(super::verify_sha256(sums, "app.deb", &file).is_ok());
        assert!(super::verify_sha256(sums, "otra.exe", &file).is_err());
        assert!(super::verify_sha256(sums, "falta.dmg", &file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
