# X-Fold
### **High-Performance Fujifilm X-Trans 2D Mathematical Compression**

**X-Fold** is a specialized utility designed to compress Fujifilm X-T5 RAW (.RAF) files. By exploiting the specific geometric properties of the **X-Trans CMOS 5 HR** sensor, it achieves significantly higher compression ratios than generic algorithms like ZIP or Zstd alone.

---

## 📸 Performance Benchmarks
*X-Fold typically beats standard ZIP/Zstd compression by approximately 30%.*

> [!IMPORTANT]
> **RAF file compressed using ZIP and X-Fold**
> <img width="1081" height="334" alt="image" src="https://github.com/user-attachments/assets/28b401dd-010d-46d8-a303-3cb9e641b7cd" />

> As you can see the compressed file produced by X-Fold is 41mb compared to the 60mb file produced by zip compression. Outperforming zip compression by just over 30%

---

## 🧠 Sensor-Aware Compression
Generic compression treats a RAW file as a flat stream of data. **X-Fold** understands the **6x6 Mosaic** unique to Fujifilm sensors.

### 1. Geometric Plane Decomposition
The engine performs a "Safety Crop" to ensure dimensions are multiples of 6. It then decomposes the raw image into **36 distinct planes**. Each plane represents pixels that share the same relative position within the 6x6 grid, ensuring the engine only compares pixels of the same color and phase.

### 2. LOCO-I Edge-Aware Prediction
Within each plane, we apply a modified **LOCO-I (Lossless Compression of Images)** algorithm. Instead of storing the raw 14-bit or 16-bit pixel value, we predict the next value based on its neighbors:
* **Edge Detection:** The algorithm detects horizontal or vertical edges to choose the best predictor (Above vs. Left).
* **Inverse Math:** The decompression logic mirrors this exactly to ensure bit-perfect restoration.

### 3. Bias Correction & ZigZag Encoding
* **Bias Table:** A 144-byte table (36 values) is calculated for the entire image to track prediction drift.
* **Bias Fix:** By subtracting the average prediction error from each plane, we center the residuals around zero.
* **ZigZag Encoding:** Signed residuals are mapped to unsigned integers ($u16$), transforming the data into a high-entropy state dominated by small values.
* **Final Compression:** This creates the ideal range of data for the final **Zstd Level 5** entropy coder.

---

## 🛠 Features
* **Bit-Perfect:** Reconstructs byte-for-byte identical `.RAF` files, including original metadata and headers.
* **Multi-threaded:** Powered by `Rayon` for parallel mathematical operations across all CPU cores.
* **Modern GUI:** Built with `egui` for a lightweight, hardware-accelerated user experience.
