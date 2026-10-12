use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VectorError {
    #[error("vector dimensions differ: {left} and {right}")]
    DimensionMismatch { left: usize, right: usize },
}

pub fn cosine(left: &[f32], right: &[f32]) -> Result<f32, VectorError> {
    if left.len() != right.len() || left.is_empty() {
        return Err(VectorError::DimensionMismatch {
            left: left.len(),
            right: right.len(),
        });
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        return Ok(0.0);
    }
    Ok(dot / (left_norm * right_norm))
}

pub fn encode_f32(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn decode_f32(bytes: &[u8]) -> Result<Vec<f32>, VectorError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(VectorError::DimensionMismatch {
            left: bytes.len(),
            right: 0,
        });
    }
    let mut values = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let mut buf = [0; 4];
        buf.copy_from_slice(chunk);
        values.push(f32::from_le_bytes(buf));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mixed_dimensions() {
        assert!(cosine(&[1.0, 0.0], &[1.0, 0.0, 0.0]).is_err());
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]).unwrap() - 1.0).abs() < 0.001);
        let encoded = encode_f32(&[1.0, 2.0]);
        assert_eq!(decode_f32(&encoded).unwrap(), vec![1.0, 2.0]);
    }
}
