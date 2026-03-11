
pub trait VectorMetric : Send + Sync {
    fn score(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError>;
}

#[derive(Debug, thiserror::Error)]
pub enum VectorMetricError {
    #[error("vector len no equal {0} != {1}")]
    VectorLen(usize, usize),
}

// ============================================================================ //
//                 cosine similarity
// ============================================================================ //

pub struct Cosine;

impl Cosine {
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError> {
        if a.len() != b.len() {
            return Err(VectorMetricError::VectorLen(a.len(), b.len()))
        }
    
        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;
    
        for (x, y) in a.iter().zip(b.iter()) {
            dot_product += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }
    
        let score = if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a.sqrt() * norm_b.sqrt())
        };

        Ok(score)
    }
}

impl VectorMetric for Cosine {
    #[inline]
    fn score(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError> {
        Self::cosine_similarity(a, b)
    }
}

// ============================================================================ //
//                 cosine similarity
// ============================================================================ //

pub struct L2;

impl L2 {
    pub fn l2_distance(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError> {
        if a.len() != b.len() {
            return Err(VectorMetricError::VectorLen(a.len(), b.len()))
        }

        let score = a.iter().zip(b.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();

        Ok(score)
    }
}

impl VectorMetric for L2 {
    #[inline]
    fn score(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError> {
        Self::l2_distance(a, b)
    }
}

// ============================================================================ //
//                 cosine similarity
// ============================================================================ //

pub struct Dot;

impl Dot {
    pub fn dot(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError> {
        if a.len() != b.len() {
            return Err(VectorMetricError::VectorLen(a.len(), b.len()))
        }

        let score = a.iter().zip(b.iter())
            .map(|(x, y)| x * y)
            .sum();

        Ok(score)
    }
}

impl VectorMetric for Dot {
    #[inline]
    fn score(a: &[f32], b: &[f32]) -> Result<f32, VectorMetricError> {
        Self::dot(a, b)
    }
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter())
        .map(|(x, y)| x * y)
        .sum()
}

