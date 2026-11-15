//! hexpatch: busca una secuencia de bytes en un archivo y la reemplaza.
//!
//! Uso: hexpatch [--dry-run] [--all] <archivo> <hex_buscar> <hex_reemplazar>
//!
//! Códigos de salida:
//!   0  parche aplicado (o, con --dry-run, coincidencia válida encontrada)
//!   1  patrón no encontrado
//!   2  error de uso, de lectura/escritura o hex inválido
//!   3  varias coincidencias y no se pasó --all (no se escribe nada)
//!   4  búsqueda y reemplazo tienen distinto tamaño (no se escribe nada)

use std::env;
use std::fs;
use std::process;

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.is_empty() || s.len() % 2 != 0 || !s.is_ascii() {
        return Err(format!("hex inválido (longitud impar o vacío): {s}"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| format!("hex inválido: {s}")))
        .collect()
}

/// Devuelve los offsets de todas las coincidencias (incluye las solapadas).
fn find_all(data: &[u8], pat: &[u8]) -> Vec<usize> {
    if pat.is_empty() || pat.len() > data.len() {
        return Vec::new();
    }
    data.windows(pat.len())
        .enumerate()
        .filter(|(_, w)| *w == pat)
        .map(|(i, _)| i)
        .collect()
}

/// Reemplaza en cada offset. Si dos coincidencias se solapan, se rechaza.
fn apply(data: &mut [u8], offsets: &[usize], pat_len: usize, rep: &[u8]) -> Result<(), String> {
    for pair in offsets.windows(2) {
        if pair[1] < pair[0] + pat_len {
            return Err("coincidencias solapadas: no se puede parchear de forma segura".into());
        }
    }
    for &off in offsets {
        data[off..off + rep.len()].copy_from_slice(rep);
    }
    Ok(())
}

fn run() -> Result<i32, String> {
    let mut dry_run = false;
    let mut all = false;
    let mut pos: Vec<String> = Vec::new();
    for a in env::args().skip(1) {
        match a.as_str() {
            "--dry-run" => dry_run = true,
            "--all" => all = true,
            _ => pos.push(a),
        }
    }
    if pos.len() != 3 {
        return Err("uso: hexpatch [--dry-run] [--all] <archivo> <hex_buscar> <hex_reemplazar>".into());
    }
    let (file, search, replace) = (&pos[0], parse_hex(&pos[1])?, parse_hex(&pos[2])?);

    if search.len() != replace.len() {
        eprintln!(
            "hexpatch: tamaños distintos (buscar {} bytes, reemplazar {} bytes); no se aplica",
            search.len(),
            replace.len()
        );
        return Ok(4);
    }

    let mut data = fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let offsets = find_all(&data, &search);

    if offsets.is_empty() {
        return Ok(1);
    }
    for off in &offsets {
        println!("coincidencia en offset 0x{off:x}");
    }
    if offsets.len() > 1 && !all {
        eprintln!(
            "hexpatch: {} coincidencias; no se aplica sin --all",
            offsets.len()
        );
        return Ok(3);
    }
    if dry_run {
        return Ok(0);
    }

    apply(&mut data, &offsets, search.len(), &replace)?;
    fs::write(file, &data).map_err(|e| format!("{file}: {e}"))?;
    println!("parcheado: {} coincidencia(s)", offsets.len());
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("hexpatch: {e}");
            process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_ok_y_invalido() {
        assert_eq!(parse_hex("00ffAb").unwrap(), vec![0x00, 0xff, 0xab]);
        assert!(parse_hex("abc").is_err());
        assert!(parse_hex("zz").is_err());
        assert!(parse_hex("").is_err());
    }

    #[test]
    fn busca_todas() {
        let d = [1, 2, 3, 1, 2, 3, 9];
        assert_eq!(find_all(&d, &[1, 2, 3]), vec![0, 3]);
        assert_eq!(find_all(&d, &[7]), Vec::<usize>::new());
        assert_eq!(find_all(&[1, 1, 1], &[1, 1]), vec![0, 1]);
    }

    #[test]
    fn aplica_y_rechaza_solapadas() {
        let mut d = vec![1, 2, 3, 1, 2, 3, 9];
        apply(&mut d, &[0, 3], 3, &[7, 8, 9]).unwrap();
        assert_eq!(d, vec![7, 8, 9, 7, 8, 9, 9]);
        let mut e = vec![1, 1, 1];
        assert!(apply(&mut e, &[0, 1], 2, &[5, 5]).is_err());
        assert_eq!(e, vec![1, 1, 1]);
    }
}
