//! Oracle for the cargo-deb fuzz harness.
//!
//! Both fuzz targets end up calling `cargo_deb::compress::xz_or_gz` (via `process()`'s
//! `ControlArchiveBuilder`/`data::generate_archive` calls) to build the `.deb` control/data
//! tarballs. This asserts that entry point's real behaviour: the produced stream must round-trip
//! back to the exact input bytes. A no-op/stub harness could not pass it.
//!
//! Run by mayhem/test.sh as `cargo test` of this crate (RUSTFLAGS cleared, no sanitizer).

use std::io::{Read, Write};

use cargo_deb::compress::xz_or_gz;

fn unxz(stream: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    xz2::read::XzDecoder::new(stream).read_to_end(&mut out).expect("unxz the cargo-deb xz stream");
    out
}

fn corpus() -> Vec<Vec<u8>> {
    vec![
        b"".to_vec(),
        b"a".to_vec(),
        b"hello world\n".to_vec(),
        b"the quick brown fox jumps over the lazy dog".repeat(64),
        (0u8..=255).cycle().take(4096).collect(),       // highly compressible ramp
        (0u32..2048).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect(), // pseudo-random / incompressible
        vec![0u8; 8192],                                  // all zeros
    ]
}

#[test]
fn xz_round_trips_fast_and_slow() {
    for fast in [true, false] {
        for input in corpus() {
            let mut c = xz_or_gz(fast, false).expect("xz compressor");
            c.write_all(&input).expect("write to xz compressor");
            let compressed = c.finish().expect("finish xz compressor");
            let stream: &[u8] = &compressed;
            assert!(!stream.is_empty(), "xz stream must never be empty");
            assert_eq!(unxz(stream), input, "xz stream must decompress to the original bytes");
        }
    }
}

#[test]
fn streamed_writes_match_single_write() {
    // cargo-deb writes the tarball to the compressor in chunks; chunked writes must produce a
    // stream equivalent (after decompression) to the whole input.
    let input: Vec<u8> = b"chunk boundaries must not corrupt the stream".repeat(100);
    let mut c = xz_or_gz(false, false).unwrap();
    for chunk in input.chunks(7) {
        c.write_all(chunk).unwrap();
    }
    let compressed = c.finish().unwrap();
    assert_eq!(unxz(&compressed), input);
}
