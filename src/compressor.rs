use rayon::prelude::*;
use std::io::Write;
use std::time::Instant;
use std::fs::{self, File};
use std::path::Path;

pub fn compress_pixels(path: &Path, width: usize, height: usize, raw_data: &[u16]) -> Vec<u8> {
    let plane_w = width / 6;
    let plane_h = height / 6;
    let plane_size = plane_w * plane_h;
    let total_pixels = width * height;

    // --- PHASE 0: EXTRACT RAF WRAPPER ---
    // We read the original file to save the metadata/headers for later reconstruction
    let original_bytes = fs::read(path).expect("Failed to read original RAF");
    let pixel_bytes_len = total_pixels * 2;
    let header_size = original_bytes.len() - pixel_bytes_len;
    let raf_header = &original_bytes[..header_size];

    let start_math = Instant::now();

    // --- PHASE 1: PARALLEL MATH (LOCO-I + Bias Correction) ---
    let mut all_residuals = vec![0u16; total_pixels];

    let biases: Vec<i32> = all_residuals
        .par_chunks_mut(plane_size)
        .enumerate()
        .map(|(plane_idx, chunk)| {
            let r_start = plane_idx / 6;
            let c_start = plane_idx % 6;

            let mut signed_residuals = vec![0i32; plane_size];
            let mut top_row = vec![0i32; plane_w];
            let mut sum_diff: i64 = 0;

            for r_count in 0..plane_h {
                let r = r_start + (r_count * 6);
                let mut left_pred = 0i32;
                let mut top_left_pred = 0i32;
                let row_offset = r * width;

                for c_count in 0..plane_w {
                    let c = c_start + (c_count * 6);
                    let current = raw_data[row_offset + c] as i32;
                    let above = top_row[c_count];

                    // Edge-Aware Predictor
                    let pred = if top_left_pred >= above.max(left_pred) {
                        above.min(left_pred)
                    } else if top_left_pred <= above.min(left_pred) {
                        above.max(left_pred)
                    } else {
                        above + left_pred - top_left_pred
                    };

                    let diff = current - pred;
                    signed_residuals[r_count * plane_w + c_count] = diff;
                    sum_diff += diff as i64;

                    top_left_pred = above;
                    left_pred = current;
                    top_row[c_count] = current;
                }
            }

            let bias = (sum_diff / plane_size as i64) as i32;

            for i in 0..plane_size {
                let corrected = signed_residuals[i] - bias;
                chunk[i] = if corrected >= 0 {
                    (corrected << 1) as u16
                } else {
                    ((-corrected << 1) - 1) as u16
                };
            }
            bias
        })
        .collect();

    println!("⏱️ Phase 1 (LOCO-I + Bias Fix): {:?}", start_math.elapsed());

    // --- PHASE 2: PACKING & ZSTD ---
    let start_zstd = Instant::now();
    let bin_bytes = unsafe {
        std::slice::from_raw_parts(all_residuals.as_ptr() as *const u8, all_residuals.len() * 2)
    };

    // 1. Prepare Payload with Header Information
    let mut final_payload = Vec::with_capacity(bin_bytes.len() / 2);
    final_payload.extend_from_slice(b"XFLD"); // Magic Tag
    final_payload.extend_from_slice(&(width as u32).to_le_bytes());
    final_payload.extend_from_slice(&(height as u32).to_le_bytes());

    // 2. Store Biases (144 bytes)
    for b in &biases {
        final_payload.extend_from_slice(&b.to_le_bytes());
    }

    // 3. Store RAF Wrapper Header
    final_payload.extend_from_slice(&(header_size as u32).to_le_bytes());
    final_payload.extend_from_slice(raf_header);

    // 4. Compress Residuals and Append
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 5).unwrap();
    encoder.multithread(0).unwrap();
    encoder.include_contentsize(true).unwrap();
    encoder.write_all(bin_bytes).unwrap();

    let compressed = encoder.finish().unwrap();
    final_payload.extend(compressed);

    println!("⏱️ Phase 2 (Zstd L5 + u16 + Metadata): {:?}", start_zstd.elapsed());
    final_payload
}