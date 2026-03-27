use std::fs::File;
use std::io::{Read, Cursor, Write};
use std::path::Path;

pub fn decompress_to_raf(path: &Path) {
    let mut file = File::open(path).expect("Failed to open vault file");

    // 1. Read Header (Magic & Dimensions)
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).unwrap(); // "XFLD"

    let mut dims = [0u8; 8];
    file.read_exact(&mut dims).unwrap();
    let width = u32::from_le_bytes(dims[0..4].try_into().unwrap()) as usize;
    let height = u32::from_le_bytes(dims[4..8].try_into().unwrap()) as usize;

    // 2. Read 144-byte Bias Table (36 i32 values)
    let mut bias_bytes = [0u8; 144];
    file.read_exact(&mut bias_bytes).unwrap();
    let mut biases = [0i32; 36];
    for i in 0..36 {
        biases[i] = i32::from_le_bytes(bias_bytes[i*4..(i+1)*4].try_into().unwrap());
    }

    // 3. Read RAF Wrapper (The original Fujifilm metadata)
    let mut h_size_bytes = [0u8; 4];
    file.read_exact(&mut h_size_bytes).unwrap();
    let header_size = u32::from_le_bytes(h_size_bytes) as usize;

    let mut raf_wrapper = vec![0u8; header_size];
    file.read_exact(&mut raf_wrapper).unwrap();

    // 4. Decompress Zstd payload into u16 residuals
    let mut compressed_payload = Vec::new();
    file.read_to_end(&mut compressed_payload).unwrap();
    let decoded_res_bytes = zstd::decode_all(Cursor::new(compressed_payload)).unwrap();

    let residuals: Vec<u16> = decoded_res_bytes.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    // 5. Invert Logic (LOCO-I + Bias Correction)
    let mut restored_pixels = vec![0u16; width * height];
    let plane_w = width / 6;
    let plane_h = height / 6;

    for plane_idx in 0..36 {
        let r_start = plane_idx / 6;
        let c_start = plane_idx % 6;
        let bias = biases[plane_idx];
        let mut top_row = vec![0i32; plane_w];

        for r_count in 0..plane_h {
            let r = r_start + (r_count * 6);
            let mut left_pred = 0i32;
            let mut top_left_pred = 0i32;

            for c_count in 0..plane_w {
                let c = c_start + (c_count * 6);
                let zz = residuals[plane_idx * (plane_w * plane_h) + (r_count * plane_w + c_count)];

                // Invert ZigZag
                let corrected_diff = if zz % 2 == 0 { (zz >> 1) as i32 } else { -(((zz + 1) >> 1) as i32) };
                let diff = corrected_diff + bias;

                // LOCO-I Inverse Prediction
                let above = top_row[c_count];
                let pred = if top_left_pred >= above.max(left_pred) {
                    above.min(left_pred)
                } else if top_left_pred <= above.min(left_pred) {
                    above.max(left_pred)
                } else {
                    above + left_pred - top_left_pred
                };

                let pixel = (pred + diff) as u16;
                restored_pixels[r * width + c] = pixel;

                // Update predictors for next pixel
                top_left_pred = above;
                left_pred = pixel as i32;
                top_row[c_count] = pixel as i32;
            }
        }
    }

    // 6. STITCH AND SAVE THE .RAF FILE
    let mut restored_raf_path = path.to_path_buf();
    restored_raf_path.set_extension("RAF");

    let mut raf_file = File::create(&restored_raf_path).expect("Failed to create restored RAF");

    // Write the original Fujifilm header/metadata
    raf_file.write_all(&raf_wrapper).unwrap();

    // Write the bit-perfect restored pixels
    let pixel_bytes = unsafe {
        std::slice::from_raw_parts(restored_pixels.as_ptr() as *const u8, restored_pixels.len() * 2)
    };
    raf_file.write_all(pixel_bytes).unwrap();

    println!("✅ Reconstructed bit-perfect RAF: {:?}", restored_raf_path);
}